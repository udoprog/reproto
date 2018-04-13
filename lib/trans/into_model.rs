use ast::*;
use attributes;
use core::errors::Error;
use core::flavored::*;
use core::{self, Attributes, BigInt, Diagnostics, Loc, Selection, Span, SymbolKind, WithSpan};
use linked_hash_map::LinkedHashMap;
use naming::Naming;
use scope::Scope;
use std::borrow::Cow;
use std::collections::{hash_map, BTreeSet, HashMap};
use std::option;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::result;

/// All error information is propagated to the diagnostics argument, but we signal that an error
/// occurred by returning Err(()).
pub type Result<T> = result::Result<T, ()>;

/// Helper macro to deal with a unit error in a loop.
///
/// Important note: must use other diagnostics facilities to determine if an error happened, like
/// `Diagnostics::has_errors`.
#[macro_export]
macro_rules! try_loop {
    ($e:expr) => {
        match $e {
            Err(()) => continue,
            Ok(ok) => ok,
        }
    };
}

/// Check for conflicting items and generate appropriate error messages if they are.
macro_rules! check_conflict {
    ($diag:expr, $existing:expr, $item:expr, $accessor:expr, $what:expr) => {
        if let Some(other) = $existing.insert($accessor.to_string(), Span::from(&$item).clone())
        {
            $diag.err(
                Span::from(&$item),
                format!(concat!($what, " `{}` is already defined"), $accessor),
            );

            $diag.info(other, "previously defined here");
            return Err(());
        }
    };
}

/// Checks if a given field matches a sub-type tag.
macro_rules! check_field_tag {
    ($diag:ident, $field:expr, $strategy:expr) => {
        match $strategy {
            core::RpSubTypeStrategy::Tagged { ref tag, .. } => {
                if $field.name() == tag {
                    $diag.err(
                        Loc::span(&$field),
                        format!(
                            "field with name `{}` is the same as tag used in type_info",
                            tag
                        ),
                    );

                    return Err(());
                }
            }
            _ => {}
        }
    };
}

macro_rules! check_field_reserved {
    ($diag:ident, $field:expr, $reserved:expr) => {
        if let Some(reserved) = $reserved.get($field.name()) {
            $diag.err(
                Loc::span(&$field),
                format!("field with name `{}` is reserved", $field.name()),
            );

            $diag.info(reserved, "reserved here");
            return Err(());
        }
    };
}

#[derive(Debug, Default)]
pub struct MemberConstraint<'input> {
    sub_type_strategy: Option<&'input RpSubTypeStrategy>,
    reserved: Option<&'input HashMap<String, Span>>,
}

#[derive(Debug)]
pub struct SubTypeConstraint<'input> {
    sub_type_strategy: &'input RpSubTypeStrategy,
    reserved: &'input HashMap<String, Span>,
    field_idents: &'input HashMap<String, Span>,
    field_names: &'input HashMap<String, Span>,
    untagged: &'input mut LinkedHashMap<BTreeSet<String>, Span>,
}

#[derive(Debug)]
pub struct Members {
    fields: Vec<Loc<RpField>>,
    codes: Vec<Loc<RpCode>>,
    decls: Vec<RpDecl>,
    field_names: HashMap<String, Span>,
    field_idents: HashMap<String, Span>,
}

/// Adds a method for all types that supports conversion into core types.
pub trait IntoModel {
    type Output;

    /// Convert the current type to a model.
    fn into_model(self, diag: &mut Diagnostics, scope: &Scope) -> Result<Self::Output>;
}

/// Generic implementation for vectors.
impl<T> IntoModel for Loc<T>
where
    T: IntoModel,
{
    type Output = Loc<T::Output>;

    fn into_model(self, diag: &mut Diagnostics, scope: &Scope) -> Result<Self::Output> {
        let (value, span) = Loc::take_pair(self);
        Ok(Loc::new(value.into_model(diag, scope)?, span))
    }
}

/// Error recovery.
impl<'input, T> IntoModel for (&'static str, Loc<ErrorRecovery<'input, T>>)
where
    T: IntoModel,
{
    type Output = T::Output;

    fn into_model(self, diag: &mut Diagnostics, scope: &Scope) -> Result<Self::Output> {
        let (message, recovery) = self;
        let (recovery, span) = Loc::take_pair(recovery);

        match recovery {
            ErrorRecovery::Error(_) => {
                diag.err(span, message);
                Err(())
            }
            ErrorRecovery::Value(value) => value.into_model(diag, scope),
        }
    }
}

/// Generic implementation for vectors.
impl<T> IntoModel for Vec<T>
where
    T: IntoModel,
{
    type Output = Vec<T::Output>;

    fn into_model(self, diag: &mut Diagnostics, scope: &Scope) -> Result<Self::Output> {
        let mut out = Vec::new();

        for v in self {
            out.push(v.into_model(diag, scope)?);
        }

        Ok(out)
    }
}

impl<T> IntoModel for Option<T>
where
    T: IntoModel,
{
    type Output = Option<T::Output>;

    fn into_model(self, diag: &mut Diagnostics, scope: &Scope) -> Result<Self::Output> {
        match self {
            Some(value) => Ok(Some(value.into_model(diag, scope)?)),
            None => Ok(None),
        }
    }
}

impl<T> IntoModel for Box<T>
where
    T: IntoModel,
{
    type Output = Box<T::Output>;

    fn into_model(self, diag: &mut Diagnostics, scope: &Scope) -> Result<Self::Output> {
        Ok(Box::new((*self).into_model(diag, scope)?))
    }
}

impl<'a> IntoModel for Cow<'a, str> {
    type Output = String;

    fn into_model(self, _: &mut Diagnostics, _scope: &Scope) -> Result<Self::Output> {
        Ok(self.to_string())
    }
}

impl IntoModel for String {
    type Output = String;

    fn into_model(self, _: &mut Diagnostics, _scope: &Scope) -> Result<Self::Output> {
        Ok(self)
    }
}

/// Helper model to strip whitespace prefixes from comment lines.
pub struct Comment<I>(I);

impl<I: IntoIterator<Item = S>, S: AsRef<str>> IntoModel for Comment<I> {
    type Output = Vec<String>;

    fn into_model(self, _: &mut Diagnostics, _scope: &Scope) -> Result<Self::Output> {
        let comment = self.0.into_iter().collect::<Vec<_>>();

        let pfx = comment
            .iter()
            .flat_map(|s| s.as_ref().find(|c: char| !c.is_whitespace()))
            .min()
            .unwrap_or(0);

        let comment: Vec<String> = comment
            .into_iter()
            .map(|s| {
                let s = s.as_ref();
                s[usize::min(s.len(), pfx)..].to_string()
            })
            .collect();

        Ok(comment)
    }
}

impl<'input> IntoModel for Type<'input> {
    type Output = RpType;

    fn into_model(self, diag: &mut Diagnostics, scope: &Scope) -> Result<RpType> {
        use self::Type::*;

        let out = match self {
            Double => core::RpType::Double,
            Float => core::RpType::Float,
            Signed { size } => core::RpType::Signed { size: size },
            Unsigned { size } => core::RpType::Unsigned { size: size },
            Boolean => core::RpType::Boolean,
            String => core::RpType::String,
            DateTime => core::RpType::DateTime,
            Name { name } => core::RpType::Name {
                name: name.into_model(diag, scope)?,
            },
            Array { inner } => core::RpType::Array {
                inner: inner.into_model(diag, scope)?,
            },
            Map { key, value } => core::RpType::Map {
                key: key.into_model(diag, scope)?,
                value: value.into_model(diag, scope)?,
            },
            Any => core::RpType::Any,
            Bytes => core::RpType::Bytes,
        };

        Ok(out)
    }
}

impl<'input> IntoModel for Decl<'input> {
    type Output = RpDecl;

    fn into_model(self, diag: &mut Diagnostics, scope: &Scope) -> Result<Self::Output> {
        use self::Decl::*;

        let s = scope.child(self.name().to_owned());

        let out = match self {
            Type(body) => core::RpDecl::Type(body.into_model(diag, &s)?),
            Interface(body) => core::RpDecl::Interface(body.into_model(diag, &s)?),
            Enum(body) => core::RpDecl::Enum(body.into_model(diag, &s)?),
            Tuple(body) => core::RpDecl::Tuple(body.into_model(diag, &s)?),
            Service(body) => core::RpDecl::Service(body.into_model(diag, &s)?),
        };

        Ok(out)
    }
}

impl<'input> IntoModel for Item<'input, EnumBody<'input>> {
    type Output = Loc<RpEnumBody>;

    fn into_model(self, diag: &mut Diagnostics, scope: &Scope) -> Result<Self::Output> {
        macro_rules! variants {
            (
                $diag:expr, $enum_type:expr, $variants:expr,
                $(($ty:ident, $out:ident, $default:expr)),*
            ) => {
            match $enum_type {
                $(
                core::RpEnumType::$ty => {
                    let mut out = Vec::new();

                    let mut idents = HashMap::new();
                    let mut values = HashMap::new();
                    let mut default = $default;

                    for v in $variants {
                        let v = try_loop!((v, &mut default).into_model(diag, scope));

                        check_conflict!($diag, idents, v, v.ident, "variant");
                        check_conflict!($diag, values, v, v.value(), "variant value");

                        out.push(v);
                    }

                    if diag.has_errors() {
                        return Err(());
                    }

                    core::RpVariants::$out { variants: out }
                }
                )*
            }
            };
        }

        let Item {
            comment,
            attributes,
            item,
        } = self;

        let (item, span) = Loc::take_pair(item);

        let name = scope.as_name();

        diag.symbol(SymbolKind::Enum, &span, &name);

        let mut codes = Vec::new();

        for member in item.members {
            match member {
                EnumMember::Code(code) => {
                    codes.push(code.into_model(diag, scope)?);
                }
            };
        }

        let enum_type = {
            let (enum_type, span) = Loc::take_pair(item.ty.into_model(diag, scope)?);

            match enum_type.as_enum_type() {
                Some(enum_type) => enum_type,
                None => {
                    diag.err(
                        span,
                        "illegal enum type, expected `string`, `u32`, `u64`, `i32`, or `i64`",
                    );
                    return Err(());
                }
            }
        };

        let variants = variants!(
            diag,
            enum_type,
            item.variants,
            (String, String, StringDefaultVariant),
            (
                U32,
                Number,
                NumberDefaultVariant::new(core::RpEnumType::U32)
            ),
            (
                U64,
                Number,
                NumberDefaultVariant::new(core::RpEnumType::U64)
            ),
            (
                I32,
                Number,
                NumberDefaultVariant::new(core::RpEnumType::I32)
            ),
            (
                I64,
                Number,
                NumberDefaultVariant::new(core::RpEnumType::I64)
            )
        );

        let attributes = attributes.into_model(diag, scope)?;
        check_attributes!(diag, attributes);

        return Ok(Loc::new(
            RpEnumBody {
                name,
                ident: item.name.to_string(),
                comment: Comment(&comment).into_model(diag, scope)?,
                decls: vec![],
                enum_type: enum_type,
                variants: variants,
                codes: codes,
            },
            span,
        ));

        struct NumberDefaultVariant {
            state: BigInt,
            enum_type: core::RpEnumType,
        }

        impl NumberDefaultVariant {
            fn new(enum_type: core::RpEnumType) -> Self {
                Self {
                    state: 0.into(),
                    enum_type,
                }
            }
        }

        impl DefaultVariant for NumberDefaultVariant {
            type Type = RpNumber;

            fn next<'input>(&mut self, _: &EnumVariant<'input>) -> result::Result<RpNumber, Error> {
                let next = self.state.clone();
                self.state = self.state.clone() + BigInt::from(1);
                let number = RpNumber::from(next);
                self.enum_type.validate_number(&number)?;
                Ok(number)
            }

            fn process(&mut self, value: RpValue) -> result::Result<RpNumber, Error> {
                let number = value.into_number()?;

                {
                    let value = number
                        .to_bigint()
                        .ok_or_else(|| "value can't be used with generator")?;

                    self.state = value.clone();
                }

                self.enum_type.validate_number(&number)?;
                Ok(number)
            }
        }

        struct StringDefaultVariant;

        impl DefaultVariant for StringDefaultVariant {
            type Type = String;

            fn next<'input>(
                &mut self,
                variant: &EnumVariant<'input>,
            ) -> result::Result<String, Error> {
                Ok(variant.name.to_string())
            }

            fn process(&mut self, value: RpValue) -> result::Result<String, Error> {
                value
                    .as_string()
                    .map(|s| s.to_string())
                    .map_err(|_| format!("expected `string`, did you mean \"{}\"?", value).into())
            }
        }
    }
}

/// Type that generates a variant value.
pub trait DefaultVariant {
    type Type;

    /// Get the next default variant value.
    fn next<'input>(&mut self, variant: &EnumVariant<'input>) -> result::Result<Self::Type, Error>;

    /// Process the value, attempting to convert it to the destination type.
    fn process(&mut self, value: RpValue) -> result::Result<Self::Type, Error>;
}

/// enum value with assigned ordinal
impl<'input, 'a, D> IntoModel for (Item<'input, EnumVariant<'input>>, &'a mut D)
where
    D: DefaultVariant,
{
    type Output = Loc<RpVariant<D::Type>>;

    fn into_model(self, diag: &mut Diagnostics, scope: &Scope) -> Result<Self::Output> {
        let (variant, default) = self;

        let Item {
            comment,
            attributes,
            item,
        } = variant;

        let (item, span) = Loc::take_pair(item);

        let value = if let Some(argument) = item.argument {
            let (value, span) = Loc::take_pair(argument.into_model(diag, scope)?);

            match default.process(value) {
                Err(e) => {
                    diag.err(span, e.display());
                    return Err(());
                }
                Ok(value) => value,
            }
        } else {
            default.next(&item).with_span(diag, span)?
        };

        let attributes = attributes.into_model(diag, scope)?;
        check_attributes!(diag, attributes);

        Ok(Loc::new(
            RpVariant {
                name: scope.as_name().push(item.name.to_string()),
                ident: Loc::map(item.name.clone(), |s| s.to_string()),
                comment: Comment(&comment).into_model(diag, scope)?,
                value: value,
            },
            span,
        ))
    }
}

/// Helper function to build a safe identifier.
fn build_safe_ident(scope: &Scope, ident: &str, naming: Option<&Naming>) -> Option<String> {
    if let Some(ident_naming) = naming {
        let converted = ident_naming.convert(ident);

        match scope.keyword(converted.as_str()) {
            Some(ident) => Some(ident.to_string()),
            None if converted.as_str() != ident => Some(converted),
            None => None,
        }
    } else {
        scope.keyword(ident).map(|s| s.to_string())
    }
}

/// Helper function to build a safe name.
fn build_item_name(
    scope: &Scope,
    ident: &str,
    name: Option<&str>,
    default_naming: Option<&Naming>,
    default_ident_naming: Option<&Naming>,
) -> (String, Option<String>, Option<String>) {
    let safe_ident = build_safe_ident(scope, ident, default_ident_naming);

    // Apply specification-wide naming convention unless field name explicitly specified.
    let name = name.map(|s| s.to_string())
        .or_else(|| default_naming.map(|n| n.convert(ident)));

    // Don't include field alias if same as name.
    let name = match name {
        // Explicit alias, but it's exactly the same as translated field.
        Some(ref name) if name == ident => None,
        // Explicit alias that differs from field.
        Some(name) => Some(name),
        // Name matches ident
        _ => None,
    };

    (ident.to_string(), safe_ident, name)
}

impl<'input> IntoModel for Item<'input, Field<'input>> {
    type Output = Loc<RpField>;

    fn into_model(self, diag: &mut Diagnostics, scope: &Scope) -> Result<Loc<RpField>> {
        let Item {
            comment,
            attributes,
            item,
        } = self;

        let (item, span) = Loc::take_pair(item);

        let field_as = item.field_as.into_model(diag, scope)?;

        let (ident, safe_ident, field_as) = build_item_name(
            scope,
            item.name.as_ref(),
            field_as.as_ref().map(|s| s.as_str()),
            scope.field_naming(),
            scope.field_ident_naming(),
        );

        let attributes = attributes.into_model(diag, scope)?;
        check_attributes!(diag, attributes);

        Ok(Loc::new(
            RpField {
                required: item.required,
                safe_ident: safe_ident,
                ident: ident,
                comment: Comment(&comment).into_model(diag, scope)?,
                ty: (
                    "expected type, like: `string`, `u32`, or `MyType`.",
                    item.ty,
                ).into_model(diag, scope)?,
                field_as: field_as,
            },
            span,
        ))
    }
}

impl<'input> IntoModel for File<'input> {
    type Output = RpFile;

    fn into_model(self, diag: &mut Diagnostics, scope: &Scope) -> Result<RpFile> {
        let mut decls = Vec::new();

        for d in self.decls {
            decls.push(try_loop!(d.into_model(diag, scope)));
        }

        if diag.has_errors() {
            return Err(());
        }

        Ok(RpFile {
            comment: Comment(&self.comment).into_model(diag, scope)?,
            decls: decls,
        })
    }
}

impl<'input> IntoModel for Item<'input, InterfaceBody<'input>> {
    type Output = Loc<RpInterfaceBody>;

    fn into_model(self, diag: &mut Diagnostics, scope: &Scope) -> Result<Self::Output> {
        let Item {
            comment,
            attributes,
            item,
        } = self;

        let (item, span) = Loc::take_pair(item);

        let name = scope.as_name();

        diag.symbol(SymbolKind::Interface, &span, &name);

        let mut attributes = attributes.into_model(diag, scope)?;

        let reserved = attributes::reserved(diag, &mut attributes)?;

        let mut sub_type_strategy = RpSubTypeStrategy::default();

        if let Some(mut type_info) = attributes.take_selection("type_info") {
            sub_type_strategy = push_type_info(diag, &mut type_info)?;
            check_selection!(diag, type_info);
        }

        check_attributes!(diag, attributes);

        let Members {
            fields,
            codes,
            decls,
            field_idents,
            field_names,
            ..
        } = {
            let constraint = MemberConstraint {
                sub_type_strategy: Some(&sub_type_strategy),
                ..MemberConstraint::default()
            };

            (item.members, constraint).into_model(diag, scope)?
        };

        let mut names = HashMap::new();
        let mut idents = HashMap::new();
        let mut sub_types = Vec::new();
        let mut untagged = LinkedHashMap::new();

        for sub_type in item.sub_types {
            let scope = scope.child(Loc::borrow(&sub_type.name).to_owned());

            let constraint = SubTypeConstraint {
                sub_type_strategy: &sub_type_strategy,
                reserved: &reserved,
                field_idents: &field_idents,
                field_names: &field_names,
                untagged: &mut untagged,
            };

            let sub_type = (sub_type, constraint).into_model(diag, &scope)?;

            check_conflict!(diag, idents, sub_type, sub_type.ident, "sub-type");
            check_conflict!(diag, names, sub_type, sub_type.name(), "sub-type with name");

            sub_types.push(sub_type);
        }

        // check that we are not violating any constraints.
        match *&sub_type_strategy {
            core::RpSubTypeStrategy::Untagged => {
                check_untagged(diag, &sub_types, &untagged)?;

                // Check that - in the order sub-types appear, any the key for any give
                // sub-type is not a subset of any sub-sequent sub-types.

                let mut it = untagged.iter();

                while let Some((k0, span0)) = it.next() {
                    let mut sub = it.clone();

                    while let Some((k1, span1)) = sub.next() {
                        if !k0.is_subset(k1) {
                            continue;
                        }

                        let names = k0.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ");

                        diag.err(
                            span0,
                            &format!(
                                "fields with names `{}` are present in another sub-type, this \
                                 would cause deserialization to be ambiguous for certain cases.",
                                names,
                            ),
                        );

                        diag.info(
                            span0,
                            "HINT: re-order or change your sub-types to avoid this",
                        );

                        let names = k1.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ");

                        diag.info(
                            span1,
                            &format!(
                                "conflicting sub-type with fields `{}` is defined here",
                                names
                            ),
                        );
                    }
                }

                if diag.has_errors() {
                    return Err(());
                }
            }
            _ => {}
        }

        return Ok(Loc::new(
            RpInterfaceBody {
                name,
                ident: item.name.to_string(),
                comment: Comment(&comment).into_model(diag, scope)?,
                decls: decls,
                fields: fields,
                codes: codes,
                sub_types: sub_types,
                sub_type_strategy: sub_type_strategy,
            },
            span,
        ));

        /// Check invariants that need to be enforced with unique fields
        fn check_untagged<'a, I: 'a>(
            diag: &mut Diagnostics,
            sub_types: &Vec<Loc<RpSubType>>,
            untagged: I,
        ) -> Result<()>
        where
            I: Clone + IntoIterator<Item = (&'a BTreeSet<String>, &'a Span)>,
        {
            for sub_type in sub_types {
                let required = sub_type
                    .fields
                    .iter()
                    .filter(|f| f.is_required())
                    .map(|f| f.name().to_string())
                    .collect::<BTreeSet<_>>();

                for (key, span) in untagged.clone() {
                    // skip own
                    if *key == required {
                        continue;
                    }

                    let mut any = false;

                    let optional = sub_type.fields.iter().filter(|f| f.is_optional());

                    for f in optional.filter(|f| key.contains(f.name())) {
                        any = true;
                        diag.err(Loc::span(f), "is a required field of another sub-type");
                    }

                    if any {
                        diag.info(span.clone(), "sub-type defined here");
                    }
                }
            }

            if diag.has_errors() {
                return Err(());
            }

            Ok(())
        }

        /// Extract type_info attribute.
        fn push_type_info(
            diag: &mut Diagnostics,
            selection: &mut Selection,
        ) -> Result<RpSubTypeStrategy> {
            if let Some(strategy) = selection.take("strategy") {
                let (strategy, span) = Loc::take_pair(strategy);
                let id = strategy.as_string().with_span(diag, span)?;

                match id {
                    "tagged" => {
                        if let Some(tag) = selection.take("tag") {
                            let (tag, span) = Loc::take_pair(tag);
                            let tag = tag.as_string().with_span(diag, span)?;

                            return Ok(core::RpSubTypeStrategy::Tagged {
                                tag: tag.to_string(),
                            });
                        }
                    }
                    "untagged" => {
                        return Ok(core::RpSubTypeStrategy::Untagged);
                    }
                    _ => {
                        diag.err(span, "bad strategy");
                        return Err(());
                    }
                }
            }

            Ok(RpSubTypeStrategy::default())
        }
    }
}

impl<'input> IntoModel for Name<'input> {
    type Output = RpName;

    fn into_model(self, diag: &mut Diagnostics, scope: &Scope) -> Result<Self::Output> {
        use self::Name::*;

        let out = match self {
            Relative { parts } => {
                let parts = parts.into_model(diag, scope)?;

                scope
                    .as_name()
                    .extend(parts.into_iter().map(|p| Loc::take(p)))
            }
            Absolute { prefix, parts } => {
                let parts = parts
                    .into_model(diag, scope)?
                    .into_iter()
                    .map(|s| Loc::take(s))
                    .collect();

                let (prefix, package) = match prefix {
                    Some(prefix) => {
                        let (prefix, span) = Loc::take_pair(prefix);

                        match scope.lookup_prefix(prefix.as_ref()) {
                            Some(package) => {
                                let prefix = prefix.to_string();
                                (Some(Loc::new(prefix, span)), package.clone())
                            }
                            None => {
                                diag.err(span, format!("missing prefix `{}`", prefix.clone()));
                                return Err(());
                            }
                        }
                    }
                    None => (None, scope.package()),
                };

                RpName {
                    prefix,
                    package,
                    parts,
                }
            }
        };

        Ok(out)
    }
}

impl<'input> IntoModel for (&'input Path, usize, usize) {
    type Output = (PathBuf, usize, usize);

    fn into_model(self, _: &mut Diagnostics, _scope: &Scope) -> Result<Self::Output> {
        Ok((self.0.to_owned(), self.1, self.2))
    }
}

impl<'input> IntoModel for Item<'input, ServiceBody<'input>> {
    type Output = Loc<RpServiceBody>;

    fn into_model(self, diag: &mut Diagnostics, scope: &Scope) -> Result<Self::Output> {
        let Item {
            comment,
            attributes,
            item,
        } = self;

        let (item, span) = Loc::take_pair(item);

        let name = scope.as_name();

        diag.symbol(SymbolKind::Service, &span, &name);

        let mut decl_idents = HashMap::new();
        let mut endpoint_names = HashMap::new();
        let mut endpoint_idents = HashMap::new();

        let mut endpoints = Vec::new();
        let mut decls = Vec::new();

        for member in item.members {
            match member {
                ServiceMember::Endpoint(e) => {
                    let e = e.into_model(diag, scope)?;

                    check_conflict!(diag, endpoint_idents, e, e.ident(), "endpoint");
                    check_conflict!(diag, endpoint_names, e, e.name(), "endpoint with name");

                    endpoints.push(e);
                }
                ServiceMember::InnerDecl(d) => {
                    let d = d.into_model(diag, scope)?;
                    check_conflict!(diag, decl_idents, d, d.ident(), "inner declaration");
                    decls.push(d);
                }
            };
        }

        let mut attributes = attributes.into_model(diag, scope)?;

        let mut http = RpServiceBodyHttp::default();

        if let Some(selection) = attributes.take_selection("http") {
            let (mut selection, _) = Loc::take_pair(selection);
            push_http(diag, &mut selection, &mut http)?;
            check_selection!(diag, selection);
        }

        check_attributes!(diag, attributes);

        return Ok(Loc::new(
            RpServiceBody {
                name,
                ident: item.name.to_string(),
                comment: Comment(&comment).into_model(diag, scope)?,
                decls: decls,
                http: http,
                endpoints: endpoints,
            },
            span,
        ));

        fn push_http(
            diag: &mut Diagnostics,
            selection: &mut Selection,
            http: &mut RpServiceBodyHttp,
        ) -> Result<()> {
            if let Some(url) = selection.take("url") {
                let (url, span) = Loc::take_pair(url);
                let url = url.as_string().with_span(diag, span)?.to_string();
                http.url = Some(Loc::new(url, span));
            }

            Ok(())
        }
    }
}

impl<'input> IntoModel for EndpointArgument<'input> {
    type Output = RpEndpointArgument;

    fn into_model(self, diag: &mut Diagnostics, scope: &Scope) -> Result<Self::Output> {
        let ident = self.ident.into_model(diag, scope)?;
        let safe_ident = build_safe_ident(scope, ident.as_str(), scope.field_ident_naming());

        let argument = RpEndpointArgument {
            ident: Rc::new(ident),
            safe_ident: Rc::new(safe_ident),
            channel: self.channel.into_model(diag, scope)?,
        };

        Ok(argument)
    }
}

impl<'input> IntoModel for Item<'input, Endpoint<'input>> {
    type Output = Loc<RpEndpoint>;

    fn into_model(self, diag: &mut Diagnostics, scope: &Scope) -> Result<Self::Output> {
        let Item {
            comment,
            attributes,
            item,
        } = self;

        let (item, span) = Loc::take_pair(item);

        let id = item.id.into_model(diag, scope)?;
        let alias = item.alias.into_model(diag, scope)?;

        let (ident, safe_ident, name) = build_item_name(
            scope,
            id.as_str(),
            alias.as_ref().map(|s| s.as_str()),
            scope.endpoint_naming(),
            scope.endpoint_ident_naming(),
        );

        let mut arguments = Vec::new();
        let mut seen = HashMap::new();

        for argument in item.arguments {
            let argument = argument.into_model(diag, scope)?;

            if let Some(other) = seen.insert(
                argument.ident.to_string(),
                Loc::span(&argument.ident).clone(),
            ) {
                diag.err(Loc::span(&argument.ident), "argument already present");
                diag.info(other, "argument present here");
                return Err(());
            }

            arguments.push(argument);
        }

        let response = item.response.into_model(diag, scope)?;
        let mut request = arguments.iter().cloned().next();

        let mut attributes = attributes.into_model(diag, scope)?;

        let http = attributes::endpoint_http(
            diag,
            scope,
            &mut attributes,
            &mut request,
            response.as_ref(),
            &arguments,
        )?;

        check_attributes!(diag, attributes);

        Ok(Loc::new(
            RpEndpoint {
                ident: ident,
                safe_ident: safe_ident,
                name: name,
                comment: Comment(&comment).into_model(diag, scope)?,
                attributes: attributes,
                arguments: arguments,
                request: request,
                response: response,
                http: http,
            },
            span,
        ))
    }
}

impl<'input> IntoModel for Channel<'input> {
    type Output = RpChannel;

    fn into_model(self, diag: &mut Diagnostics, scope: &Scope) -> Result<Self::Output> {
        use self::Channel::*;

        let result = match self {
            Unary { ty, .. } => core::RpChannel::Unary {
                ty: ty.into_model(diag, scope)?,
            },
            Streaming { ty, .. } => core::RpChannel::Streaming {
                ty: ty.into_model(diag, scope)?,
            },
        };

        Ok(result)
    }
}

impl<'input> IntoModel for (Item<'input, SubType<'input>>, SubTypeConstraint<'input>) {
    type Output = Loc<RpSubType>;

    fn into_model(self, diag: &mut Diagnostics, scope: &Scope) -> Result<Self::Output> {
        use self::TypeMember::*;

        let (item, constraint) = self;

        let SubTypeConstraint {
            reserved: interface_reserved,
            field_idents,
            field_names,
            sub_type_strategy,
            untagged,
        } = constraint;

        let Item {
            comment,
            attributes,
            item,
        } = item;

        let (item, span) = Loc::take_pair(item);

        let name = scope.as_name();

        let mut attributes = attributes.into_model(diag, scope)?;
        let reserved = attributes::reserved(diag, &mut attributes)?;
        check_attributes!(diag, attributes);

        let mut fields = Vec::new();
        let mut codes = Vec::new();
        let mut decls = Vec::new();

        let mut decl_idents = HashMap::new();
        let mut field_idents = field_idents.clone();
        let mut field_names = field_names.clone();

        for member in item.members {
            match member {
                Field(field) => {
                    let field = field.into_model(diag, scope)?;

                    check_conflict!(diag, field_idents, field, field.ident(), "field");
                    check_conflict!(diag, field_names, field, field.name(), "field with name");

                    check_field_tag!(diag, field, *sub_type_strategy);

                    check_field_reserved!(diag, field, interface_reserved);
                    check_field_reserved!(diag, field, reserved);

                    fields.push(field);
                }
                Code(code) => {
                    codes.push(code.into_model(diag, scope)?);
                }
                InnerDecl(d) => {
                    let d = d.into_model(diag, scope)?;
                    check_conflict!(diag, decl_idents, d, d.ident(), "inner declaration");
                    decls.push(d);
                }
            }
        }

        let sub_type_name = sub_type_name(diag, item.alias, scope)?;

        match *sub_type_strategy {
            core::RpSubTypeStrategy::Untagged => {
                let fields = fields
                    .iter()
                    .filter(|f| f.is_required())
                    .map(|f| f.name().to_string())
                    .collect::<BTreeSet<_>>();

                if let Some(other) = untagged.insert(fields, span.clone()) {
                    diag.err(span, "does not have a unique set of fields");
                    diag.info(other, "previously defined here");
                    return Err(());
                }
            }
            _ => {}
        }

        return Ok(Loc::new(
            RpSubType {
                name,
                ident: item.name.to_string(),
                comment: Comment(&comment).into_model(diag, scope)?,
                decls: decls,
                fields: fields,
                codes: codes,
                sub_type_name: sub_type_name,
            },
            span,
        ));

        /// Extract all names provided.
        fn alias_name<'input>(
            diag: &mut Diagnostics,
            alias: Loc<Value<'input>>,
            scope: &Scope,
        ) -> Result<Loc<String>> {
            let (alias, span) = Loc::take_pair(alias.into_model(diag, scope)?);

            match alias {
                core::RpValue::String(string) => Ok(Loc::new(string, span)),
                _ => {
                    diag.err(span, "expected string");
                    return Err(());
                }
            }
        }

        fn sub_type_name<'input>(
            diag: &mut Diagnostics,
            alias: option::Option<Loc<Value<'input>>>,
            scope: &Scope,
        ) -> Result<::std::option::Option<Loc<String>>> {
            if let Some(alias) = alias {
                alias_name(diag, alias, scope).map(Some)
            } else {
                Ok(None)
            }
        }
    }
}

impl<'input> IntoModel for Item<'input, TupleBody<'input>> {
    type Output = Loc<RpTupleBody>;

    fn into_model(self, diag: &mut Diagnostics, scope: &Scope) -> Result<Self::Output> {
        let Item {
            comment,
            attributes,
            item,
        } = self;

        let (item, span) = Loc::take_pair(item);

        let name = scope.as_name();

        diag.symbol(SymbolKind::Tuple, &span, &name);

        let Members {
            fields,
            codes,
            decls,
            ..
        } = item.members.into_model(diag, scope)?;

        let attributes = attributes.into_model(diag, scope)?;
        check_attributes!(diag, attributes);

        Ok(Loc::new(
            RpTupleBody {
                name: name,
                ident: item.name.to_string(),
                comment: Comment(&comment).into_model(diag, scope)?,
                decls: decls,
                fields: fields,
                codes: codes,
            },
            span,
        ))
    }
}

impl<'input> IntoModel for Item<'input, TypeBody<'input>> {
    type Output = Loc<RpTypeBody>;

    fn into_model(self, diag: &mut Diagnostics, scope: &Scope) -> Result<Self::Output> {
        let Item {
            comment,
            attributes,
            item,
        } = self;

        let (item, span) = Loc::take_pair(item);

        let name = scope.as_name();

        diag.symbol(SymbolKind::Type, &span, &name);

        let mut attributes = attributes.into_model(diag, scope)?;
        let reserved = attributes::reserved(diag, &mut attributes)?;

        check_attributes!(diag, attributes);

        let Members {
            fields,
            codes,
            decls,
            ..
        } = {
            let constraint = MemberConstraint {
                reserved: Some(&reserved),
                ..MemberConstraint::default()
            };

            (item.members, constraint).into_model(diag, scope)?
        };

        Ok(Loc::new(
            RpTypeBody {
                name,
                ident: item.name.to_string(),
                comment: Comment(&comment).into_model(diag, scope)?,
                decls: decls,
                fields: fields,
                codes: codes,
            },
            span,
        ))
    }
}

/// Default constraints.
impl<'input> IntoModel for Vec<TypeMember<'input>> {
    type Output = Members;

    fn into_model(self, diag: &mut Diagnostics, scope: &Scope) -> Result<Self::Output> {
        (self, MemberConstraint::default()).into_model(diag, scope)
    }
}

impl<'input> IntoModel for (Vec<TypeMember<'input>>, MemberConstraint<'input>) {
    type Output = Members;

    fn into_model(self, diag: &mut Diagnostics, scope: &Scope) -> Result<Self::Output> {
        use self::TypeMember::*;

        let (members, constraint) = self;

        let MemberConstraint {
            sub_type_strategy,
            reserved,
        } = constraint;

        let mut fields: Vec<Loc<RpField>> = Vec::new();
        let mut codes = Vec::new();
        let mut decls = Vec::new();

        let mut field_idents = HashMap::new();
        let mut field_names = HashMap::new();
        let mut decl_idents = HashMap::new();

        for member in members {
            match member {
                Field(field) => {
                    let field = field.into_model(diag, scope)?;

                    check_conflict!(diag, field_idents, field, field.ident(), "field");
                    check_conflict!(diag, field_names, field, field.name(), "field with name");

                    if let Some(sub_type_strategy) = sub_type_strategy {
                        check_field_tag!(diag, field, *sub_type_strategy);
                    }

                    if let Some(reserved) = reserved {
                        check_field_reserved!(diag, field, reserved);
                    }

                    fields.push(field);
                }
                Code(code) => codes.push(code.into_model(diag, scope)?),
                InnerDecl(d) => {
                    let d = d.into_model(diag, scope)?;
                    check_conflict!(diag, decl_idents, d, d.ident(), "inner declaration");
                    decls.push(d);
                }
            }
        }

        Ok(Members {
            fields: fields,
            codes: codes,
            decls: decls,
            field_names: field_names,
            field_idents: field_idents,
        })
    }
}

impl<'input> IntoModel for Code<'input> {
    type Output = RpCode;

    fn into_model(self, diag: &mut Diagnostics, scope: &Scope) -> Result<Self::Output> {
        let mut attributes = self.attributes.into_model(diag, scope)?;
        let context = self.context.into_model(diag, scope)?;

        // Context-specific settings.
        let context = {
            let (context, span) = Loc::take_pair(context);

            match context.as_str() {
                "csharp" => core::RpContext::Csharp {},
                "go" => core::RpContext::Go {},
                "java" => {
                    let imports = attributes::import(diag, &mut attributes)?;
                    core::RpContext::Java { imports: imports }
                }
                "js" => core::RpContext::Js {},
                "python" => core::RpContext::Python {},
                "reproto" => core::RpContext::Reproto {},
                "rust" => core::RpContext::Rust {},
                "swift" => core::RpContext::Swift {},
                context => {
                    diag.err(span, format!("context `{}` not recognized", context));
                    return Err(());
                }
            }
        };

        check_attributes!(diag, attributes);

        Ok(RpCode {
            context: context,
            lines: self.content.into_iter().map(|s| s.to_string()).collect(),
        })
    }
}

impl<'input> IntoModel for Value<'input> {
    type Output = RpValue;

    fn into_model(self, diag: &mut Diagnostics, scope: &Scope) -> Result<RpValue> {
        use self::Value::*;

        let out = match self {
            String(string) => core::RpValue::String(string),
            Number(number) => core::RpValue::Number(number),
            Identifier(identifier) => core::RpValue::Identifier(identifier.to_string()),
            Array(inner) => core::RpValue::Array(inner.into_model(diag, scope)?),
        };

        Ok(out)
    }
}

impl<'input> IntoModel for Vec<Loc<Attribute<'input>>> {
    type Output = Attributes;

    fn into_model(self, diag: &mut Diagnostics, scope: &Scope) -> Result<Attributes> {
        use self::Attribute::*;

        let mut words = HashMap::new();
        let mut selections = HashMap::new();

        for attribute in self {
            let (attr, attr_pos) = Loc::take_pair(attribute);

            match attr {
                Word(word) => {
                    let (word, span) = Loc::take_pair(word.into_model(diag, scope)?);

                    if let Some(old) = words.insert(word, span.clone()) {
                        diag.err(span, "word already present");
                        diag.info(old, "old attribute here");
                        return Err(());
                    }
                }
                List(key, name_values) => {
                    let key = Loc::take(key.into_model(diag, scope)?);

                    match selections.entry(key) {
                        hash_map::Entry::Vacant(entry) => {
                            let mut words = Vec::new();
                            let mut values = HashMap::new();

                            for name_value in name_values {
                                match name_value {
                                    AttributeItem::Word(word) => {
                                        words.push(word.into_model(diag, scope)?);
                                    }
                                    AttributeItem::NameValue { name, value } => {
                                        let name = name.into_model(diag, scope)?;
                                        let value = value.into_model(diag, scope)?;
                                        values.insert(Loc::borrow(&name).clone(), (name, value));
                                    }
                                }
                            }

                            let selection = Selection::new(words, values);
                            entry.insert(Loc::new(selection, attr_pos));
                        }
                        hash_map::Entry::Occupied(entry) => {
                            diag.err(attr_pos, "attribute already present");
                            diag.info(Loc::span(entry.get()), "attribute here");
                            return Err(());
                        }
                    }
                }
            }
        }

        Ok(Attributes::new(words, selections))
    }
}

#[allow(unused)]
type Variables<'a> = HashMap<&'a str, &'a RpEndpointArgument>;

impl<'input, 'a: 'input> IntoModel for (Span, &'input mut Variables<'a>, PathSpec<'input>) {
    type Output = RpPathSpec;

    fn into_model(self, diag: &mut Diagnostics, scope: &Scope) -> Result<Self::Output> {
        let (span, vars, spec) = self;

        let mut out = Vec::new();

        for s in spec.steps {
            out.push((span, &mut *vars, s).into_model(diag, scope)?);
        }

        Ok(RpPathSpec { steps: out })
    }
}

impl<'input, 'a: 'input> IntoModel for (Span, &'input mut Variables<'a>, PathStep<'input>) {
    type Output = RpPathStep;

    fn into_model(self, diag: &mut Diagnostics, scope: &Scope) -> Result<Self::Output> {
        let (span, vars, step) = self;

        let mut out = Vec::new();

        for p in step.parts {
            out.push((span, &mut *vars, p).into_model(diag, scope)?);
        }

        Ok(RpPathStep { parts: out })
    }
}

impl<'input, 'a: 'input> IntoModel for (Span, &'input mut Variables<'a>, PathPart<'input>) {
    type Output = RpPathPart;

    fn into_model(self, diag: &mut Diagnostics, scope: &Scope) -> Result<Self::Output> {
        let (span, vars, part) = self;

        use self::PathPart::*;

        let out = match part {
            Variable(variable) => {
                let var = variable.into_model(diag, scope)?;

                let var = match vars.remove(var.as_str()) {
                    Some(rp) => rp.clone(),
                    None => {
                        diag.err(
                            span,
                            format!("path variable `{}` is not an argument to endpoint", var),
                        );

                        return Err(());
                    }
                };

                core::RpPathPart::Variable(var)
            }
            Segment(segment) => core::RpPathPart::Segment(segment),
        };

        Ok(out)
    }
}
