use super::errors::*;
use super::scope::Scope;
pub use core::*;
pub use parser::ast::*;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// Adds a method for all types that supports conversion into core types.
pub trait IntoModel {
    type Output;

    /// Convert the current type to a model.
    fn into_model(self, scope: &Scope) -> Result<Self::Output>;
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
            Name { name } => RpType::Name { name: name.into_model(scope)? },
            Array { inner } => RpType::Array { inner: inner.into_model(scope)? },
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

impl<'input> IntoModel for EnumBody<'input> {
    type Output = RpEnumBody;

    fn into_model(self, scope: &Scope) -> Result<Self::Output> {
        let mut variants: Vec<Rc<Loc<RpEnumVariant>>> = Vec::new();

        let mut ordinals = OrdinalGenerator::new();

        let (fields, codes, options, decls) = members_into_model(scope, self.members)?;

        let ty = self.ty.into_model(scope)?;

        let variant_type = if let Some(ty) = ty {
            ty.and_then(|ty| {
                ty.as_enum_type().ok_or_else(
                    || "expeced string or number".into(),
                ) as Result<RpEnumType>
            })?
        } else {
            RpEnumType::Number
        };

        for variant in self.variants {
            let (variant, pos) = variant.take_pair();

            let variant = (variant, &variant_type, &mut ordinals)
                .into_model(scope)
                .with_pos(&pos)?;

            if let Some(other) = variants.iter().find(
                |v| *v.local_name == *variant.local_name,
            )
            {
                return Err(
                    ErrorKind::EnumVariantConflict(
                        other.local_name.pos().into(),
                        variant.local_name.pos().into(),
                    ).into(),
                );
            }

            variants.push(Rc::new(Loc::new(variant, pos)));
        }

        let options = Options::new(options);

        let serialized_as: Option<Loc<String>> =
            options.find_one_identifier("serialized_as")?.to_owned();

        let serialized_as_name = options
            .find_one_boolean("serialized_as_name")?
            .to_owned()
            .map(|t| t.take())
            .unwrap_or(false);

        Ok(RpEnumBody {
            name: scope.as_name(),
            local_name: self.name.to_string(),
            comment: self.comment.into_iter().map(ToOwned::to_owned).collect(),
            decls: decls,
            variant_type: variant_type,
            variants: variants,
            fields: fields,
            codes: codes,
            serialized_as: serialized_as,
            serialized_as_name: serialized_as_name,
        })
    }
}

/// enum value with assigned ordinal
impl<'input, 'a> IntoModel for (EnumVariant<'input>, &'a RpEnumType, &'a mut OrdinalGenerator) {
    type Output = RpEnumVariant;

    fn into_model(self, scope: &Scope) -> Result<Self::Output> {
        let (variant, ty, ordinals) = self;

        let ordinal = if let Some(argument) = variant.argument.into_model(scope)? {
            if !ty.is_assignable_from(&argument) {
                return Err(
                    format!("unexpected value {}, expected type {}", argument, ty).into(),
                );
            }

            argument.and_then(|value| value.to_ordinal())?
        } else {
            RpEnumOrdinal::Number(ordinals.next()?)
        };

        Ok(RpEnumVariant {
            name: scope.as_name().push(variant.name.to_string()),
            local_name: variant.name.clone().map(str::to_string),
            comment: variant.comment.into_iter().map(ToOwned::to_owned).collect(),
            ordinal: ordinal,
        })
    }
}

impl<'input> IntoModel for FieldInit<'input> {
    type Output = RpFieldInit;

    fn into_model(self, scope: &Scope) -> Result<RpFieldInit> {
        let field_init = RpFieldInit {
            name: self.name.into_model(scope)?,
            value: self.value.into_model(scope)?,
        };

        Ok(field_init)
    }
}

impl<'input> IntoModel for Field<'input> {
    type Output = RpField;

    fn into_model(self, scope: &Scope) -> Result<RpField> {
        Ok(RpField::new(
            self.modifier,
            self.name.to_owned(),
            self.comment.into_iter().map(ToOwned::to_owned).collect(),
            self.ty.into_model(scope)?,
            self.field_as.into_model(scope)?,
        ))
    }
}

impl<'input> IntoModel for File<'input> {
    type Output = RpFile;

    fn into_model(self, scope: &Scope) -> Result<RpFile> {
        let options = Options::new(self.options.into_model(scope)?);

        Ok(RpFile {
            options: options,
            decls: self.decls.into_model(scope)?,
        })
    }
}

impl<'input> IntoModel for Instance<'input> {
    type Output = RpInstance;

    fn into_model(self, scope: &Scope) -> Result<RpInstance> {
        let instance = RpInstance {
            name: self.name.into_model(scope)?,
            arguments: self.arguments.into_model(scope)?,
        };

        Ok(instance)
    }
}

impl<'input> IntoModel for InterfaceBody<'input> {
    type Output = RpInterfaceBody;

    fn into_model(self, scope: &Scope) -> Result<Self::Output> {
        use std::collections::btree_map::Entry::*;

        let (fields, codes, options, decls) = members_into_model(scope, self.members)?;

        let mut sub_types: BTreeMap<String, Rc<Loc<RpSubType>>> = BTreeMap::new();

        for sub_type in self.sub_types {
            let (sub_type, pos) = sub_type.take_pair();
            let sub_type = Rc::new(Loc::new(sub_type.into_model(scope)?, pos));

            // key has to be owned by entry
            let key = sub_type.local_name.clone();

            match sub_types.entry(key) {
                Occupied(entry) => {
                    entry.into_mut().merge(sub_type)?;
                }
                Vacant(entry) => {
                    entry.insert(sub_type);
                }
            }
        }

        let _options = Options::new(options);

        Ok(RpInterfaceBody {
            name: scope.as_name(),
            local_name: self.name.to_string(),
            comment: self.comment.into_iter().map(ToOwned::to_owned).collect(),
            decls: decls,
            fields: fields,
            codes: codes,
            sub_types: sub_types,
        })
    }
}

/// Generic implementation for vectors.
impl<T> IntoModel for Loc<T>
where
    T: IntoModel,
{
    type Output = Loc<T::Output>;

    fn into_model(self, scope: &Scope) -> Result<Self::Output> {
        let (value, pos) = self.take_pair();
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

impl<'a> IntoModel for &'a str {
    type Output = String;

    fn into_model(self, _scope: &Scope) -> Result<Self::Output> {
        Ok(self.to_owned())
    }
}

impl IntoModel for String {
    type Output = String;

    fn into_model(self, _scope: &Scope) -> Result<Self::Output> {
        Ok(self)
    }
}

impl IntoModel for RpPackage {
    type Output = RpPackage;

    fn into_model(self, _scope: &Scope) -> Result<Self::Output> {
        Ok(self)
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
                        return Err(ErrorKind::MissingPrefix(prefix.clone()).into());
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

impl<'input> IntoModel for Creator<'input> {
    type Output = RpCreator;

    fn into_model(self, scope: &Scope) -> Result<RpCreator> {
        use self::Creator::*;

        let out = match self {
            Instance(instance) => RpCreator::Instance(instance.into_model(scope)?),
            Constant(constant) => RpCreator::Constant(constant.into_model(scope)?),
        };

        Ok(out)
    }
}

impl<'input> IntoModel for OptionDecl<'input> {
    type Output = RpOptionDecl;

    fn into_model(self, scope: &Scope) -> Result<RpOptionDecl> {
        let decl = RpOptionDecl {
            name: self.name.to_owned(),
            values: self.values.into_model(scope)?,
        };

        Ok(decl)
    }
}

impl<'input> IntoModel for PathSegment<'input> {
    type Output = RpPathSegment;

    fn into_model(self, scope: &Scope) -> Result<RpPathSegment> {
        let out = match self {
            PathSegment::Literal { value } => RpPathSegment::Literal {
                value: value.into_model(scope)?,
            },
            PathSegment::Variable { name, ty } => {
                RpPathSegment::Variable {
                    name: name.into_model(scope)?,
                    ty: ty.into_model(scope)?,
                }
            }
        };

        Ok(out)
    }
}

impl<'input> IntoModel for PathSpec<'input> {
    type Output = RpPathSpec;

    fn into_model(self, scope: &Scope) -> Result<RpPathSpec> {
        Ok(RpPathSpec { segments: self.segments.into_model(scope)? })
    }
}

struct Node {
    parent: Option<Rc<RefCell<Node>>>,
    method: Option<Loc<String>>,
    path: Option<Loc<RpPathSpec>>,
    options: Vec<Loc<RpOptionDecl>>,
    comment: Vec<String>,
    returns: Vec<RpServiceReturns>,
    accepts: Vec<RpServiceAccepts>,
}

impl Node {
    fn push_returns(&mut self, input: RpServiceReturns) {
        self.returns.push(input);
    }

    fn push_accepts(&mut self, input: RpServiceAccepts) {
        self.accepts.push(input);
    }
}

fn convert_return(
    scope: &Scope,
    comment: Vec<String>,
    status: Option<Loc<RpNumber>>,
    produces: Option<Loc<String>>,
    ty: Option<Loc<Type>>,
    options: Vec<Loc<OptionDecl>>,
) -> Result<RpServiceReturns> {
    let options = Options::new(options.into_model(scope)?);

    let produces = produces.or(options.find_one_string("produces")?);

    let produces = if let Some(produces) = produces {
        let (produces, pos) = produces.take_pair();

        let produces = produces.parse().chain_err(|| {
            ErrorKind::Pos("not a valid mime type".to_owned(), pos.into())
        })?;

        Some(produces)
    } else {
        None
    };

    let status = status.or(options.find_one_number("status")?);

    let status = if let Some(status) = status {
        let (status, pos) = status.take_pair();

        let status = status.to_u32().ok_or_else(|| {
            ErrorKind::Pos("not a valid status".to_owned(), pos.into())
        })?;

        Some(status)
    } else {
        None
    };

    Ok(RpServiceReturns {
        comment: comment,
        ty: ty.into_model(scope)?,
        produces: produces,
        status: status,
    })
}

fn convert_accepts(
    scope: &Scope,
    comment: Vec<String>,
    accepts: Option<Loc<String>>,
    alias: Option<Loc<String>>,
    ty: Option<Loc<Type>>,
    options: Vec<Loc<OptionDecl>>,
) -> Result<RpServiceAccepts> {
    let options = Options::new(options.into_model(scope)?);

    let accepts = accepts.or(options.find_one_string("accept")?);

    let accepts = if let Some(accepts) = accepts {
        let (accepts, pos) = accepts.take_pair();

        let accepts = accepts.parse().chain_err(|| {
            ErrorKind::Pos("not a valid mime type".to_owned(), pos.into())
        })?;

        Some(accepts)
    } else {
        None
    };

    Ok(RpServiceAccepts {
        comment: comment,
        ty: ty.into_model(scope)?,
        accepts: accepts,
        alias: alias,
    })
}

/// Recursively unwind all inherited information about the given node, and convert to a service
/// endpoint.
fn unwind(node: Rc<RefCell<Node>>) -> Result<RpServiceEndpoint> {
    let mut method: Option<Loc<String>> = None;
    let mut path = Vec::new();
    let mut options: Vec<Loc<RpOptionDecl>> = Vec::new();
    let mut returns = Vec::new();
    let mut accepts = Vec::new();

    let comment = node.try_borrow()?.comment.clone();

    let mut current = Some(node);

    while let Some(step) = current {
        let next = step.try_borrow()?;

        // set method if not set
        method = method.or_else(|| next.method.clone());

        if let Some(ref next_url) = next.path {
            // correct order by extending in reverse
            path.extend(next_url.as_ref().segments.iter().rev().map(Clone::clone));
        }

        options.extend(next.options.iter().map(Clone::clone).rev());
        returns.extend(next.returns.iter().map(Clone::clone));
        accepts.extend(next.accepts.iter().map(Clone::clone));

        current = next.parent.clone();
    }

    let path = RpPathSpec { segments: path.into_iter().rev().collect() };

    let _options = Options::new(options.into_iter().rev().collect());

    Ok(RpServiceEndpoint {
        method: method,
        path: path,
        comment: comment,
        returns: returns,
        accepts: accepts,
    })
}

impl<'input> IntoModel for ServiceBody<'input> {
    type Output = RpServiceBody;

    fn into_model(self, scope: &Scope) -> Result<Self::Output> {
        let mut endpoints: Vec<RpServiceEndpoint> = Vec::new();

        // collecting root declarations
        let root = Rc::new(RefCell::new(Node {
            parent: None,
            method: None,
            path: None,
            options: Vec::new(),
            comment: Vec::new(),
            returns: Vec::new(),
            accepts: Vec::new(),
        }));

        let mut queue = Vec::new();
        queue.push((root, self.children));

        while let Some((parent, children)) = queue.pop() {
            for child in children {
                process_child(scope, &mut queue, &parent, child)?;
            }

            let p = parent.as_ref().try_borrow()?;

            if p.method.is_some() {
                endpoints.push(unwind(parent.clone())?);
            }
        }

        let endpoints = endpoints.into_iter().rev().collect();

        return Ok(RpServiceBody {
            name: scope.as_name(),
            local_name: self.name.to_string(),
            comment: self.comment.into_iter().map(ToOwned::to_owned).collect(),
            endpoints: endpoints,
            decls: vec![],
        });

        fn process_child<'input>(
            scope: &Scope,
            queue: &mut Vec<(Rc<RefCell<Node>>, Vec<ServiceNested<'input>>)>,
            parent: &Rc<RefCell<Node>>,
            child: ServiceNested<'input>,
        ) -> Result<()> {
            match child {
                ServiceNested::Endpoint {
                    method,
                    path,
                    comment,
                    options,
                    children,
                } => {
                    let node = Rc::new(RefCell::new(Node {
                        parent: Some(parent.clone()),
                        method: method.into_model(scope)?,
                        path: path.into_model(scope)?,
                        options: options.into_model(scope)?,
                        comment: comment.into_iter().map(ToOwned::to_owned).collect(),
                        returns: Vec::new(),
                        accepts: Vec::new(),
                    }));

                    queue.push((node, children));
                }
                // end node, manifest an endpoint.
                ServiceNested::Returns {
                    comment,
                    status,
                    produces,
                    ty,
                    options,
                } => {
                    let comment = comment.into_iter().map(ToOwned::to_owned).collect();
                    let returns = convert_return(scope, comment, status, produces, ty, options)?;
                    parent.try_borrow_mut()?.push_returns(returns);
                }
                ServiceNested::Accepts {
                    comment,
                    accepts,
                    alias,
                    ty,
                    options,
                } => {
                    let comment = comment.into_iter().map(ToOwned::to_owned).collect();
                    let alias = alias.into_model(scope)?;
                    let accepts = convert_accepts(scope, comment, accepts, alias, ty, options)?;
                    parent.try_borrow_mut()?.push_accepts(accepts);
                }
            }

            Ok(())
        }
    }
}

impl<'input> IntoModel for SubType<'input> {
    type Output = RpSubType;

    fn into_model(self, scope: &Scope) -> Result<Self::Output> {
        use self::Member::*;

        let mut fields: Vec<Loc<RpField>> = Vec::new();
        let mut codes = Vec::new();
        let mut options = Vec::new();
        let mut decls = Vec::new();

        for member in self.members {
            let (member, pos) = member.take_pair();

            match member {
                Field(field) => {
                    let field = field.into_model(scope)?;

                    if let Some(other) = fields.iter().find(|f| {
                        f.name() == field.name() || f.ident() == field.ident()
                    })
                    {
                        return Err(
                            ErrorKind::FieldConflict(
                                field.ident().to_owned(),
                                pos.into(),
                                other.pos().into(),
                            ).into(),
                        );
                    }

                    fields.push(Loc::new(field, pos));
                }
                Code(context, lines) => {
                    codes.push(code(pos, context.to_owned(), lines));
                }
                Option(option) => {
                    options.push(Loc::new(option.into_model(scope)?, pos));
                }
                InnerDecl(decl) => {
                    decls.push(Rc::new(Loc::new(decl.into_model(scope)?, pos)));
                }
            }
        }

        let options = Options::new(options);

        let names = options.find_all_strings("name")?;

        let comment = self.comment.into_iter().map(ToOwned::to_owned).collect();

        Ok(RpSubType {
            name: scope.as_name().push(self.name.to_string()),
            local_name: self.name.to_string(),
            comment: comment,
            decls: decls,
            fields: fields,
            codes: codes,
            names: names,
        })
    }
}

impl<'input> IntoModel for TupleBody<'input> {
    type Output = RpTupleBody;

    fn into_model(self, scope: &Scope) -> Result<Self::Output> {
        let (fields, codes, options, decls) = members_into_model(scope, self.members)?;

        let _options = Options::new(options);

        Ok(RpTupleBody {
            name: scope.as_name(),
            local_name: self.name.to_string(),
            comment: self.comment.into_iter().map(ToOwned::to_owned).collect(),
            decls: decls,
            fields: fields,
            codes: codes,
        })
    }
}

impl<'input> IntoModel for TypeBody<'input> {
    type Output = RpTypeBody;

    fn into_model(self, scope: &Scope) -> Result<Self::Output> {
        let (fields, codes, options, decls) = members_into_model(scope, self.members)?;

        let options = Options::new(options);

        let reserved: HashSet<Loc<String>> = options
            .find_all_identifiers("reserved")?
            .into_iter()
            .collect();

        Ok(RpTypeBody {
            name: scope.as_name(),
            local_name: self.name.to_string(),
            comment: self.comment.into_iter().map(ToOwned::to_owned).collect(),
            decls: decls,
            fields: fields,
            codes: codes,
            reserved: reserved,
        })
    }
}

type Fields = Vec<Loc<RpField>>;
type Codes = Vec<Loc<RpCode>>;
type OptionVec = Vec<Loc<RpOptionDecl>>;

pub fn code(pos: Pos, context: String, lines: Vec<String>) -> Loc<RpCode> {
    let code = RpCode {
        context: context,
        lines: lines,
    };

    Loc::new(code, pos)
}

pub fn members_into_model(
    scope: &Scope,
    members: Vec<Loc<Member>>,
) -> Result<(Fields, Codes, OptionVec, Vec<Rc<Loc<RpDecl>>>)> {
    use self::Member::*;

    let mut fields: Vec<Loc<RpField>> = Vec::new();
    let mut codes = Vec::new();
    let mut options: Vec<Loc<RpOptionDecl>> = Vec::new();
    let mut decls = Vec::new();

    for member in members {
        let (value, pos) = member.take_pair();

        match value {
            Field(field) => {
                let field = field.into_model(scope)?;

                if let Some(other) = fields.iter().find(|f| {
                    f.name() == field.name() || f.ident() == field.ident()
                })
                {
                    return Err(
                        ErrorKind::FieldConflict(
                            field.ident().to_owned(),
                            pos.into(),
                            other.pos().into(),
                        ).into(),
                    );
                }

                fields.push(Loc::new(field, pos));
            }
            Code(context, lines) => {
                codes.push(code(pos.into(), context.to_owned(), lines));
            }
            Option(option) => {
                options.push(Loc::new(option.into_model(scope)?, pos));
            }
            InnerDecl(decl) => {
                decls.push(Rc::new(Loc::new(decl.into_model(scope)?, pos)));
            }
        }
    }

    Ok((fields, codes, options, decls))
}

/// Generate ordinal values.
pub struct OrdinalGenerator {
    next_ordinal: u32,
}

impl OrdinalGenerator {
    pub fn new() -> OrdinalGenerator {
        OrdinalGenerator { next_ordinal: 0 }
    }

    pub fn next(&mut self) -> Result<u32> {
        let o = self.next_ordinal;
        self.next_ordinal += 1;
        Ok(o)
    }
}

impl<'input> IntoModel for Value<'input> {
    type Output = RpValue;

    fn into_model(self, scope: &Scope) -> Result<RpValue> {
        let out = match self {
            Value::String(string) => RpValue::String(string),
            Value::Number(number) => RpValue::Number(number),
            Value::Boolean(boolean) => RpValue::Boolean(boolean),
            Value::Identifier(identifier) => RpValue::Identifier(identifier.to_owned()),
            Value::Array(inner) => RpValue::Array(inner.into_model(scope)?),
            Value::Creator(creator) => RpValue::Creator(creator.into_model(scope)?),
        };

        Ok(out)
    }
}
