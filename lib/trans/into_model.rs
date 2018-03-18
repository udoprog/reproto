use ast::*;
use core::*;
use core::errors::{Error, Result};
use linked_hash_map::{self, LinkedHashMap};
use naming::Naming;
use scope::Scope;
use std::borrow::Cow;
use std::collections::{hash_map, HashMap};
use std::option;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use attributes;

macro_rules! check_defined {
    ($ctx:expr, $names:expr, $item:expr, $accessor:expr, $what:expr) => {
        if let Some(other) = $names.insert($accessor.to_string(), Loc::pos(&$item).clone()) {
            return Err($ctx.report()
              .err(Loc::pos(&$item), format!(concat!($what, " `{}` is already defined"), $accessor))
              .info(other, "previously defined here")
              .into());
        }
    }
}

macro_rules! check_field {
    ($ctx:ident, $field:expr, $strategy:expr) => {
        match $strategy {
            RpSubTypeStrategy::Tagged { ref tag, .. } => {
                if $field.name() == tag {
                    let report = $ctx.report()
                        .err(
                            Loc::pos(&$field),
                            format!("field name `{}` is the same as tag used in type_info", tag),
                        )
                        .into();

                    return Err(report);
                }
            }
        }
    };
}

macro_rules! check_field_reserved {
    ($ctx:ident, $field:expr, $reserved:expr) => {
        if let Some(reserved) = $reserved.get($field.name()) {
            return Err($ctx.report()
                .err(
                    Loc::pos(&$field),
                    format!("field with name `{}` is reserved", $field.name()),
                )
                .info(reserved, "reserved here")
                .into());
        }
    }
}

#[derive(Debug, Default)]
pub struct MemberConstraint<'input> {
    sub_type_strategy: Option<&'input RpSubTypeStrategy>,
    reserved: Option<&'input HashMap<String, Pos>>,
}

#[derive(Debug)]
pub struct SubTypeConstraint<'input> {
    sub_type_strategy: &'input RpSubTypeStrategy,
    reserved: &'input HashMap<String, Pos>,
    field_idents: &'input HashMap<String, Pos>,
    field_names: &'input HashMap<String, Pos>,
}

#[derive(Debug)]
pub struct Members {
    fields: Vec<Loc<RpField>>,
    codes: Vec<Loc<RpCode>>,
    decls: Vec<RpDecl>,
    field_names: HashMap<String, Pos>,
    field_idents: HashMap<String, Pos>,
}

/// Adds a method for all types that supports conversion into core types.
pub trait IntoModel {
    type Output;

    /// Convert the current type to a model.
    fn into_model(self, scope: &Scope) -> Result<Self::Output>;
}

/// Generic implementation for vectors.
impl<T> IntoModel for Loc<T>
where
    T: IntoModel,
{
    type Output = Loc<T::Output>;

    fn into_model(self, scope: &Scope) -> Result<Self::Output> {
        let (value, pos) = Loc::take_pair(self);
        Ok(Loc::new(value.into_model(scope)?, pos))
    }
}

/// Generic implementation for vectors.
impl<T> IntoModel for Vec<T>
where
    T: IntoModel,
{
    type Output = Vec<T::Output>;

    fn into_model(self, scope: &Scope) -> Result<Self::Output> {
        let mut out = Vec::new();

        for v in self {
            out.push(v.into_model(scope)?);
        }

        Ok(out)
    }
}

impl<T> IntoModel for Option<T>
where
    T: IntoModel,
{
    type Output = Option<T::Output>;

    fn into_model(self, scope: &Scope) -> Result<Self::Output> {
        if let Some(value) = self {
            return Ok(Some(value.into_model(scope)?));
        }

        Ok(None)
    }
}

impl<T> IntoModel for Box<T>
where
    T: IntoModel,
{
    type Output = Box<T::Output>;

    fn into_model(self, scope: &Scope) -> Result<Self::Output> {
        Ok(Box::new((*self).into_model(scope)?))
    }
}

impl<'a> IntoModel for Cow<'a, str> {
    type Output = String;

    fn into_model(self, _scope: &Scope) -> Result<Self::Output> {
        Ok(self.to_string())
    }
}

impl IntoModel for String {
    type Output = String;

    fn into_model(self, _scope: &Scope) -> Result<Self::Output> {
        Ok(self)
    }
}

/// Helper model to strip whitespace prefixes from comment lines.
pub struct Comment<I>(I);

impl<I: IntoIterator<Item = S>, S: AsRef<str>> IntoModel for Comment<I> {
    type Output = Vec<String>;

    fn into_model(self, _scope: &Scope) -> Result<Self::Output> {
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

impl IntoModel for Type {
    type Output = RpType;

    fn into_model(self, scope: &Scope) -> Result<RpType> {
        use self::Type::*;

        let out = match self {
            Double => RpType::Double,
            Float => RpType::Float,
            Signed { size } => RpType::Signed { size: size },
            Unsigned { size } => RpType::Unsigned { size: size },
            Boolean => RpType::Boolean,
            String => RpType::String,
            DateTime => RpType::DateTime,
            Name { name } => RpType::Name {
                name: name.into_model(scope)?,
            },
            Array { inner } => RpType::Array {
                inner: inner.into_model(scope)?,
            },
            Map { key, value } => RpType::Map {
                key: key.into_model(scope)?,
                value: value.into_model(scope)?,
            },
            Any => RpType::Any,
            Bytes => RpType::Bytes,
        };

        Ok(out)
    }
}

impl<'input> IntoModel for Decl<'input> {
    type Output = RpDecl;

    fn into_model(self, scope: &Scope) -> Result<Self::Output> {
        use self::Decl::*;

        let s = scope.child(self.name().to_owned());

        let out = match self {
            Type(body) => RpDecl::Type(Rc::new(body.into_model(&s)?)),
            Interface(body) => RpDecl::Interface(Rc::new(body.into_model(&s)?)),
            Enum(body) => RpDecl::Enum(Rc::new(body.into_model(&s)?)),
            Tuple(body) => RpDecl::Tuple(Rc::new(body.into_model(&s)?)),
            Service(body) => RpDecl::Service(Rc::new(body.into_model(&s)?)),
        };

        Ok(out)
    }
}

impl<'input> IntoModel for Item<'input, EnumBody<'input>> {
    type Output = Loc<RpEnumBody>;

    fn into_model(self, scope: &Scope) -> Result<Self::Output> {
        self.map(|comment, attributes, item| {
            let ctx = scope.ctx();

            let mut variants: Vec<Rc<Loc<RpVariant>>> = Vec::new();

            let mut codes = Vec::new();

            for member in item.members {
                match member {
                    EnumMember::Code(code) => {
                        codes.push(code.into_model(scope)?);
                    }
                };
            }

            let ty = item.ty.into_model(scope)?;

            let enum_type = Loc::take(Loc::and_then(ty, |ty| {
                ty.as_enum_type()
                    .ok_or_else(|| "illegal enum type, expected `string`".into())
                    as Result<RpEnumType>
            })?);

            let mut idents = HashMap::new();
            let mut ordinals = HashMap::new();

            for variant in item.variants {
                let variant = (variant, &enum_type).into_model(scope)?;
                check_defined!(ctx, idents, variant, variant.ident, "variant");
                check_defined!(ctx, ordinals, variant, variant.ordinal(), "variant ordinal");
                variants.push(Rc::new(variant));
            }

            let attributes = attributes.into_model(scope)?;
            check_attributes!(scope.ctx(), attributes);

            Ok(RpEnumBody {
                name: scope.as_name(),
                ident: item.name.to_string(),
                comment: Comment(&comment).into_model(scope)?,
                decls: vec![],
                enum_type: enum_type,
                variants: variants,
                codes: codes,
            })
        })
    }
}

/// enum value with assigned ordinal
impl<'input, 'a> IntoModel for (Item<'input, EnumVariant<'input>>, &'a RpEnumType) {
    type Output = Loc<RpVariant>;

    fn into_model(self, scope: &Scope) -> Result<Self::Output> {
        let (variant, ty) = self;

        let ctx = scope.ctx();

        variant.map(|comment, attributes, item| {
            let ordinal = if let Some(argument) = item.argument.into_model(scope)? {
                match *ty {
                    RpEnumType::String if !argument.is_string() => {
                        return Err(ctx.report()
                            .err(
                                Loc::pos(&argument),
                                format!("expected `{}`, did you mean \"{}\"?", ty, argument),
                            )
                            .into());
                    }
                    _ => {}
                }

                Loc::take(Loc::and_then(argument, |value| value.into_ordinal())?)
            } else {
                RpEnumOrdinal::Generated
            };

            let attributes = attributes.into_model(scope)?;
            check_attributes!(ctx, attributes);

            Ok(RpVariant {
                name: scope.as_name().push(item.name.to_string()),
                ident: Loc::map(item.name.clone(), |s| s.to_string()),
                comment: Comment(&comment).into_model(scope)?,
                ordinal: ordinal,
            })
        })
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
    let converted_ident = if let Some(ident_naming) = default_ident_naming {
        ident_naming.convert(ident)
    } else {
        ident.to_string()
    };

    // Identifier would translate to a language-specific keyword, introduce replacement
    // here.
    let safe_ident = scope
        .keyword(converted_ident.as_str())
        .map(|s| s.to_string());

    // Apply specification-wide naming convention unless field name explicitly specified.
    let name = name.map(|s| s.to_string())
        .or_else(|| default_naming.map(|n| n.convert(ident)));

    // Don't include field alias if same as name.
    let name = match name {
        // Explicit alias, but it's exactly the same as translated field.
        Some(ref name) if name == converted_ident.as_str() => None,
        // Explicit alias that differs from field.
        Some(name) => Some(name),
        // Name needs to be set if identifier does not match converted.
        _ if ident != converted_ident.as_str() => Some(ident.to_string()),
        // Name matches converted_ident
        _ => None,
    };

    (converted_ident, safe_ident, name)
}

impl<'input> IntoModel for Item<'input, Field<'input>> {
    type Output = Loc<RpField>;

    fn into_model(self, scope: &Scope) -> Result<Loc<RpField>> {
        self.map(|comment, attributes, item| {
            let field_as = item.field_as.into_model(scope)?;

            let (ident, safe_ident, field_as) = build_item_name(
                scope,
                item.name.as_ref(),
                field_as.as_ref().map(|s| s.as_str()),
                scope.field_naming(),
                scope.field_ident_naming(),
            );

            let attributes = attributes.into_model(scope)?;
            check_attributes!(scope.ctx(), attributes);

            Ok(RpField {
                required: item.required,
                safe_ident: safe_ident,
                ident: ident,
                comment: Comment(&comment).into_model(scope)?,
                ty: item.ty.into_model(scope)?,
                field_as: field_as,
            })
        })
    }
}

impl<'input> IntoModel for File<'input> {
    type Output = RpFile;

    fn into_model(self, scope: &Scope) -> Result<RpFile> {
        let decls = self.decls.into_model(scope)?;

        Ok(RpFile {
            comment: Comment(&self.comment).into_model(scope)?,
            decls: decls,
        })
    }
}

impl<'input> IntoModel for Item<'input, InterfaceBody<'input>> {
    type Output = Loc<RpInterfaceBody>;

    fn into_model(self, scope: &Scope) -> Result<Self::Output> {
        self.map(|comment, attributes, item| {
            let ctx = scope.ctx();

            let mut attributes = attributes.into_model(scope)?;

            let reserved = attributes::reserved(scope, &mut attributes)?;

            let mut sub_type_strategy = RpSubTypeStrategy::default();

            if let Some(mut type_info) = attributes.take_selection("type_info") {
                sub_type_strategy = push_type_info(ctx, &mut type_info)?;
                check_selection!(scope.ctx(), type_info);
            }

            check_attributes!(scope.ctx(), attributes);

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

                (item.members, constraint).into_model(scope)?
            };

            let mut names = HashMap::new();
            let mut idents = HashMap::new();
            let mut sub_types = Vec::new();

            for sub_type in item.sub_types {
                let scope = scope.child(Loc::value(&sub_type.name).to_owned());

                let constraint = SubTypeConstraint {
                    sub_type_strategy: &sub_type_strategy,
                    reserved: &reserved,
                    field_idents: &field_idents,
                    field_names: &field_names,
                };

                let sub_type = (sub_type, constraint).into_model(&scope)?;
                check_defined!(ctx, idents, sub_type, sub_type.ident, "sub-type");
                check_defined!(ctx, names, sub_type, sub_type.name(), "sub-type name");
                sub_types.push(Rc::new(sub_type));
            }

            return Ok(RpInterfaceBody {
                name: scope.as_name(),
                ident: item.name.to_string(),
                comment: Comment(&comment).into_model(scope)?,
                decls: decls,
                fields: fields,
                codes: codes,
                sub_types: sub_types,
                sub_type_strategy: sub_type_strategy,
            });

            /// Extract type_info attribute.
            fn push_type_info(
                ctx: &Context,
                selection: &mut Selection,
            ) -> Result<RpSubTypeStrategy> {
                if let Some(strategy) = selection.take("strategy") {
                    let id = strategy.as_string()?;

                    match id {
                        "tagged" => {
                            if let Some(tag) = selection.take("tag") {
                                let tag = tag.as_string()?;

                                return Ok(RpSubTypeStrategy::Tagged {
                                    tag: tag.to_string(),
                                });
                            }
                        }
                        _ => {
                            return Err(ctx.report()
                                .err(Loc::pos(&strategy), "bad strategy")
                                .into());
                        }
                    }
                }

                Ok(RpSubTypeStrategy::default())
            }
        })
    }
}

impl IntoModel for Name {
    type Output = RpName;

    fn into_model(self, scope: &Scope) -> Result<Self::Output> {
        use self::Name::*;

        let out = match self {
            Relative { parts } => scope.as_name().extend(parts),
            Absolute { prefix, parts } => {
                let package = if let Some(ref prefix) = prefix {
                    if let Some(package) = scope.lookup_prefix(prefix) {
                        package.clone()
                    } else {
                        return Err(Error::new(format!("Missing prefix: {}", prefix.clone())));
                    }
                } else {
                    scope.package()
                };

                RpName {
                    prefix: prefix,
                    package: package,
                    parts: parts,
                }
            }
        };

        Ok(out)
    }
}

impl<'input> IntoModel for (&'input Path, usize, usize) {
    type Output = (PathBuf, usize, usize);

    fn into_model(self, _scope: &Scope) -> Result<Self::Output> {
        Ok((self.0.to_owned(), self.1, self.2))
    }
}

impl<'input> IntoModel for Item<'input, ServiceBody<'input>> {
    type Output = Loc<RpServiceBody>;

    fn into_model(self, scope: &Scope) -> Result<Self::Output> {
        use linked_hash_map::Entry::*;

        return self.map(|comment, attributes, item| {
            let ctx = scope.ctx();

            let mut endpoint_names = HashMap::new();
            let mut endpoints = LinkedHashMap::new();
            let mut decls = Vec::new();

            for member in item.members {
                match member {
                    ServiceMember::Endpoint(endpoint) => {
                        handle_endpoint(endpoint, scope, &mut endpoint_names, &mut endpoints)?;
                    }
                    ServiceMember::InnerDecl(decl) => {
                        decls.push(decl.into_model(scope)?);
                    }
                };
            }

            let mut attributes = attributes.into_model(scope)?;

            let mut http = RpServiceBodyHttp::default();

            if let Some(selection) = attributes.take_selection("http") {
                let (mut selection, pos) = Loc::take_pair(selection);
                push_http(ctx, scope, &mut selection, &mut http).with_pos(pos)?;
                check_selection!(scope.ctx(), selection);
            }

            check_attributes!(scope.ctx(), attributes);

            Ok(RpServiceBody {
                name: scope.as_name(),
                ident: item.name.to_string(),
                comment: Comment(&comment).into_model(scope)?,
                decls: decls,
                http: http,
                endpoints: endpoints,
            })
        });

        fn push_http(
            _ctx: &Context,
            _scope: &Scope,
            selection: &mut Selection,
            http: &mut RpServiceBodyHttp,
        ) -> Result<()> {
            if let Some(url) = selection.take("url") {
                let url = Loc::and_then(url, |url| url.as_string().map(ToOwned::to_owned))?;
                http.url = Some(url);
            }

            Ok(())
        }

        /// Handle a single endpoint.
        fn handle_endpoint<'input>(
            endpoint: Item<'input, Endpoint<'input>>,
            scope: &Scope,
            names: &mut HashMap<String, Pos>,
            endpoints: &mut LinkedHashMap<String, Loc<RpEndpoint>>,
        ) -> Result<()> {
            let ctx = scope.ctx();

            let endpoint = endpoint.into_model(scope)?;

            // Check that there are no conflicting endpoint names.
            check_defined!(ctx, names, endpoint, endpoint.name(), "endpoint name");

            // Check that there are no conflicting endpoint IDs.
            match endpoints.entry(endpoint.ident().to_string()) {
                Vacant(entry) => entry.insert(endpoint),
                Occupied(entry) => {
                    return Err(ctx.report()
                        .err(Loc::pos(&endpoint), "conflicting id of endpoint")
                        .info(Loc::pos(entry.get()), "previous id here")
                        .into());
                }
            };

            Ok(())
        }
    }
}

impl<'input> IntoModel for Item<'input, Endpoint<'input>> {
    type Output = Loc<RpEndpoint>;

    fn into_model(self, scope: &Scope) -> Result<Self::Output> {
        return self.map(|comment, attributes, item| {
            let ctx = scope.ctx();

            let id = item.id.into_model(scope)?;
            let alias = item.alias.into_model(scope)?;

            let (ident, safe_ident, name) = build_item_name(
                scope,
                id.as_str(),
                alias.as_ref().map(|s| s.as_str()),
                scope.endpoint_naming(),
                None,
            );

            let mut arguments = LinkedHashMap::new();

            for (name, channel) in item.arguments {
                let name = name.into_model(scope)?;
                let channel = channel.into_model(scope)?;

                match arguments.entry(Loc::value(&name).clone()) {
                    linked_hash_map::Entry::Vacant(entry) => {
                        entry.insert((name, channel));
                    }
                    linked_hash_map::Entry::Occupied(entry) => {
                        return Err(ctx.report()
                            .err(Loc::pos(&name), "argument already present")
                            .info(Loc::pos(&entry.get().0), "argument present here")
                            .into());
                    }
                }
            }

            let response = item.response.into_model(scope)?;

            let mut attributes = attributes.into_model(scope)?;

            let http =
                attributes::endpoint_http(scope, &mut attributes, response.as_ref(), &arguments)?;

            check_attributes!(scope.ctx(), attributes);

            Ok(RpEndpoint {
                ident: ident,
                safe_ident: safe_ident,
                name: name,
                comment: Comment(&comment).into_model(scope)?,
                attributes: attributes,
                arguments: arguments,
                response: response,
                http: http,
            })
        });
    }
}

impl<'input> IntoModel for Channel {
    type Output = RpChannel;

    fn into_model(self, scope: &Scope) -> Result<Self::Output> {
        use self::Channel::*;

        let result = match self {
            Unary { ty, .. } => RpChannel::Unary {
                ty: ty.into_model(scope)?,
            },
            Streaming { ty, .. } => RpChannel::Streaming {
                ty: ty.into_model(scope)?,
            },
        };

        Ok(result)
    }
}

impl<'input> IntoModel for (Item<'input, SubType<'input>>, SubTypeConstraint<'input>) {
    type Output = Loc<RpSubType>;

    fn into_model(self, scope: &Scope) -> Result<Self::Output> {
        use self::TypeMember::*;

        let (item, constraint) = self;

        let SubTypeConstraint {
            reserved: interface_reserved,
            field_idents,
            field_names,
            sub_type_strategy,
        } = constraint;

        return item.map(|comment, attributes, item| {
            let ctx = scope.ctx();

            let mut attributes = attributes.into_model(scope)?;
            let reserved = attributes::reserved(scope, &mut attributes)?;
            check_attributes!(ctx, attributes);

            let mut fields = Vec::new();
            let mut codes = Vec::new();
            let mut decls = Vec::new();

            let mut field_idents = field_idents.clone();
            let mut field_names = field_names.clone();

            for member in item.members {
                match member {
                    Field(field) => {
                        let field = field.into_model(scope)?;
                        check_defined!(ctx, field_idents, field, field.ident(), "field");
                        check_defined!(ctx, field_names, field, field.name(), "field name");

                        check_field!(ctx, field, *sub_type_strategy);

                        check_field_reserved!(ctx, field, interface_reserved);
                        check_field_reserved!(ctx, field, reserved);

                        fields.push(field);
                    }
                    Code(code) => {
                        codes.push(code.into_model(scope)?);
                    }
                    InnerDecl(decl) => {
                        decls.push(decl.into_model(scope)?);
                    }
                }
            }

            let sub_type_name = sub_type_name(item.alias, scope)?;

            Ok(RpSubType {
                name: scope.as_name(),
                ident: item.name.to_string(),
                comment: Comment(&comment).into_model(scope)?,
                decls: decls,
                fields: fields,
                codes: codes,
                sub_type_name: sub_type_name,
            })
        });

        /// Extract all names provided.
        fn alias_name<'input>(alias: Loc<Value<'input>>, scope: &Scope) -> Result<Loc<String>> {
            let (alias, pos) = Loc::take_pair(alias.into_model(scope)?);

            match alias {
                RpValue::String(string) => Ok(Loc::new(string, pos)),
                _ => Err("expected string".into()).with_pos(pos),
            }
        }

        fn sub_type_name<'input>(
            alias: option::Option<Loc<Value<'input>>>,
            scope: &Scope,
        ) -> Result<::std::option::Option<Loc<String>>> {
            if let Some(alias) = alias {
                alias_name(alias, scope).map(Some)
            } else {
                Ok(None)
            }
        }
    }
}

impl<'input> IntoModel for Item<'input, TupleBody<'input>> {
    type Output = Loc<RpTupleBody>;

    fn into_model(self, scope: &Scope) -> Result<Self::Output> {
        self.map(|comment, attributes, item| {
            let Members {
                fields,
                codes,
                decls,
                ..
            } = item.members.into_model(scope)?;

            let attributes = attributes.into_model(scope)?;
            check_attributes!(scope.ctx(), attributes);

            Ok(RpTupleBody {
                name: scope.as_name(),
                ident: item.name.to_string(),
                comment: Comment(&comment).into_model(scope)?,
                decls: decls,
                fields: fields,
                codes: codes,
            })
        })
    }
}

impl<'input> IntoModel for Item<'input, TypeBody<'input>> {
    type Output = Loc<RpTypeBody>;

    fn into_model(self, scope: &Scope) -> Result<Self::Output> {
        self.map(|comment, attributes, item| {
            let mut attributes = attributes.into_model(scope)?;
            let reserved = attributes::reserved(scope, &mut attributes)?;

            check_attributes!(scope.ctx(), attributes);

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

                (item.members, constraint).into_model(scope)?
            };

            Ok(RpTypeBody {
                name: scope.as_name(),
                ident: item.name.to_string(),
                comment: Comment(&comment).into_model(scope)?,
                decls: decls,
                fields: fields,
                codes: codes,
            })
        })
    }
}

/// Default constraints.
impl<'input> IntoModel for Vec<TypeMember<'input>> {
    type Output = Members;

    fn into_model(self, scope: &Scope) -> Result<Self::Output> {
        (self, MemberConstraint::default()).into_model(scope)
    }
}

impl<'input> IntoModel for (Vec<TypeMember<'input>>, MemberConstraint<'input>) {
    type Output = Members;

    fn into_model(self, scope: &Scope) -> Result<Self::Output> {
        use self::TypeMember::*;

        let (members, constraint) = self;

        let MemberConstraint {
            sub_type_strategy,
            reserved,
        } = constraint;

        let ctx = scope.ctx();

        let mut fields: Vec<Loc<RpField>> = Vec::new();
        let mut codes = Vec::new();
        let mut decls = Vec::new();

        let mut field_idents = HashMap::new();
        let mut field_names = HashMap::new();

        for member in members {
            match member {
                Field(field) => {
                    let field = field.into_model(scope)?;

                    check_defined!(ctx, field_idents, field, field.ident(), "field");
                    check_defined!(ctx, field_names, field, field.name(), "field name");

                    if let Some(sub_type_strategy) = sub_type_strategy {
                        check_field!(ctx, field, *sub_type_strategy);
                    }

                    if let Some(reserved) = reserved {
                        check_field_reserved!(ctx, field, reserved);
                    }

                    fields.push(field);
                }
                Code(code) => codes.push(code.into_model(scope)?),
                InnerDecl(decl) => decls.push(decl.into_model(scope)?),
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

    fn into_model(self, scope: &Scope) -> Result<Self::Output> {
        Ok(RpCode {
            context: self.context.into_model(scope)?,
            lines: self.content.into_iter().map(|s| s.to_string()).collect(),
        })
    }
}

impl<'input> IntoModel for Value<'input> {
    type Output = RpValue;

    fn into_model(self, scope: &Scope) -> Result<RpValue> {
        use self::Value::*;

        let out = match self {
            String(string) => RpValue::String(string),
            Number(number) => RpValue::Number(number),
            Identifier(identifier) => RpValue::Identifier(identifier.to_string()),
            Array(inner) => RpValue::Array(inner.into_model(scope)?),
        };

        Ok(out)
    }
}

impl<'input> IntoModel for Vec<Loc<Attribute<'input>>> {
    type Output = Attributes;

    fn into_model(self, scope: &Scope) -> Result<Attributes> {
        use self::Attribute::*;

        let ctx = scope.ctx();

        let mut words = HashMap::new();
        let mut selections = HashMap::new();

        for attribute in self {
            let (attr, attr_pos) = Loc::take_pair(attribute);

            match attr {
                Word(word) => {
                    let (word, pos) = Loc::take_pair(word.into_model(scope)?);

                    if let Some(old) = words.insert(word, pos.clone()) {
                        return Err(ctx.report()
                            .err(pos, "word already present")
                            .info(old, "old attribute here")
                            .into());
                    }
                }
                List(key, name_values) => {
                    let key = Loc::take(key.into_model(scope)?);

                    match selections.entry(key) {
                        hash_map::Entry::Vacant(entry) => {
                            let mut words = Vec::new();
                            let mut values = HashMap::new();

                            for name_value in name_values {
                                match name_value {
                                    AttributeItem::Word(word) => {
                                        words.push(word.into_model(scope)?);
                                    }
                                    AttributeItem::NameValue { name, value } => {
                                        let name = name.into_model(scope)?;
                                        let value = value.into_model(scope)?;
                                        values.insert(Loc::value(&name).clone(), (name, value));
                                    }
                                }
                            }

                            let selection = Selection::new(words, values);
                            entry.insert(Loc::new(selection, attr_pos));
                        }
                        hash_map::Entry::Occupied(entry) => {
                            return Err(ctx.report()
                                .err(attr_pos, "attribute already present")
                                .info(Loc::pos(entry.get()), "attribute here")
                                .into());
                        }
                    }
                }
            }
        }

        Ok(Attributes::new(words, selections))
    }
}

impl<'input> IntoModel for PathSpec<'input> {
    type Output = RpPathSpec;

    fn into_model(self, scope: &Scope) -> Result<Self::Output> {
        Ok(RpPathSpec {
            steps: self.steps.into_model(scope)?,
        })
    }
}

impl<'input> IntoModel for PathStep<'input> {
    type Output = RpPathStep;

    fn into_model(self, scope: &Scope) -> Result<Self::Output> {
        Ok(RpPathStep {
            parts: self.parts.into_model(scope)?,
        })
    }
}

impl<'input> IntoModel for PathPart<'input> {
    type Output = RpPathPart;

    fn into_model(self, scope: &Scope) -> Result<Self::Output> {
        use self::PathPart::*;

        let out = match self {
            Variable(variable) => RpPathPart::Variable(variable.into_model(scope)?),
            Segment(segment) => RpPathPart::Segment(segment),
        };

        Ok(out)
    }
}
