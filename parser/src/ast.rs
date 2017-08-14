pub use core::*;
use errors::*;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[derive(Debug, PartialEq, Clone)]
pub enum Type {
    Double,
    Float,
    Signed { size: Option<usize> },
    Unsigned { size: Option<usize> },
    Boolean,
    String,
    Bytes,
    Any,
    Name { name: Name },
    Array { inner: Box<Type> },
    Map { key: Box<Type>, value: Box<Type> },
}

impl IntoModel for Type {
    type Output = RpType;

    fn into_model(self) -> Result<RpType> {
        use self::Type::*;

        let out = match self {
            Double => RpType::Double,
            Float => RpType::Float,
            Signed { size } => RpType::Signed { size: size },
            Unsigned { size } => RpType::Unsigned { size: size },
            Boolean => RpType::Boolean,
            String => RpType::String,
            Name { name } => RpType::Name { name: name.into_model()? },
            Array { inner } => RpType::Array { inner: inner.into_model()? },
            Map { key, value } => RpType::Map {
                key: key.into_model()?,
                value: value.into_model()?,
            },
            Any => RpType::Any,
            Bytes => RpType::Bytes,
        };

        Ok(out)
    }
}

#[derive(Debug)]
pub enum Decl<'input> {
    Type(TypeBody<'input>),
    Tuple(TupleBody<'input>),
    Interface(InterfaceBody<'input>),
    Enum(EnumBody<'input>),
    Service(ServiceBody<'input>),
}

impl<'input> IntoModel for Decl<'input> {
    type Output = RpDecl;

    fn into_model(self) -> Result<RpDecl> {
        use self::Decl::*;

        let decl = match self {
            Type(body) => RpDecl::Type(body.into_model()?),
            Interface(body) => RpDecl::Interface(body.into_model()?),
            Enum(body) => RpDecl::Enum(body.into_model()?),
            Tuple(body) => RpDecl::Tuple(body.into_model()?),
            Service(body) => RpDecl::Service(body.into_model()?),
        };

        Ok(decl)
    }
}

#[derive(Debug)]
pub struct EnumBody<'input> {
    pub name: &'input str,
    pub comment: Vec<&'input str>,
    pub variants: Vec<Loc<EnumVariant<'input>>>,
    pub members: Vec<Loc<Member<'input>>>,
}

impl<'input> IntoModel for EnumBody<'input> {
    type Output = Rc<RpEnumBody>;

    fn into_model(self) -> Result<Rc<RpEnumBody>> {
        let mut variants: Vec<Loc<Rc<RpEnumVariant>>> = Vec::new();

        let mut ordinals = OrdinalGenerator::new();

        let (fields, codes, options, match_decl) = members_into_model(self.members)?;

        for variant in self.variants {
            let (variant, variant_pos) = variant.both();
            let pos = &variant_pos;

            let ordinal = ordinals.next(&variant.ordinal).chain_err(|| {
                ErrorKind::Pos("failed to generate ordinal".to_owned(), pos.into())
            })?;

            if fields.len() != variant.arguments.len() {
                return Err(
                    ErrorKind::Pos(format!("expected {} arguments", fields.len()), pos.into())
                        .into(),
                );
            }

            let variant = Loc::new((variant, ordinal).into_model()?, pos.clone());

            if let Some(other) = variants.iter().find(|v| *v.name == *variant.name) {
                return Err(
                    ErrorKind::EnumVariantConflict(
                        other.name.pos().into(),
                        variant.name.pos().into(),
                    ).into(),
                );
            }

            variants.push(variant);
        }

        let options = Options::new(options);

        let serialized_as: Option<Loc<String>> =
            options.find_one_identifier("serialized_as")?.to_owned();

        let serialized_as_name = options
            .find_one_boolean("serialized_as_name")?
            .to_owned()
            .map(|t| t.move_inner())
            .unwrap_or(false);

        let en = RpEnumBody {
            name: self.name.to_owned(),
            comment: self.comment.into_iter().map(ToOwned::to_owned).collect(),
            variants: variants,
            fields: fields,
            codes: codes,
            match_decl: match_decl,
            serialized_as: serialized_as,
            serialized_as_name: serialized_as_name,
        };

        Ok(Rc::new(en))
    }
}

#[derive(Debug)]
pub struct EnumVariant<'input> {
    pub name: Loc<&'input str>,
    pub comment: Vec<&'input str>,
    pub arguments: Vec<Loc<Value<'input>>>,
    pub ordinal: Option<Loc<Value<'input>>>,
}

/// enum value with assigned ordinal
impl<'input> IntoModel for (EnumVariant<'input>, u32) {
    type Output = Rc<RpEnumVariant>;

    fn into_model(self) -> Result<Self::Output> {
        let value = self.0;
        let ordinal = self.1;

        let value = RpEnumVariant {
            name: value.name.into_model()?,
            comment: value.comment.into_iter().map(ToOwned::to_owned).collect(),
            arguments: value.arguments.into_model()?,
            ordinal: ordinal,
        };

        Ok(Rc::new(value))
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct FieldInit<'input> {
    pub name: Loc<&'input str>,
    pub value: Loc<Value<'input>>,
}

impl<'input> IntoModel for FieldInit<'input> {
    type Output = RpFieldInit;

    fn into_model(self) -> Result<RpFieldInit> {
        let field_init = RpFieldInit {
            name: self.name.into_model()?,
            value: self.value.into_model()?,
        };

        Ok(field_init)
    }
}

#[derive(Debug)]
pub struct Field<'input> {
    pub modifier: RpModifier,
    pub name: &'input str,
    pub comment: Vec<&'input str>,
    pub ty: Type,
    pub field_as: Option<Loc<Value<'input>>>,
}

impl<'input> Field<'input> {
    pub fn is_optional(&self) -> bool {
        match self.modifier {
            RpModifier::Optional => true,
            _ => false,
        }
    }
}

impl<'input> IntoModel for Field<'input> {
    type Output = RpField;

    fn into_model(self) -> Result<RpField> {
        let field_as = self.field_as.into_model()?;

        let field_as = if let Some(field_as) = field_as {
            match field_as.both() {
                (RpValue::String(name), pos) => Some(Loc::new(name, pos)),
                (_, pos) => {
                    return Err(
                        ErrorKind::Pos("must be a string".to_owned(), pos.into()).into(),
                    )
                }
            }
        } else {
            None
        };

        let name = self.name.to_owned();
        let comment = self.comment.into_iter().map(ToOwned::to_owned).collect();

        Ok(RpField::new(
            self.modifier,
            name,
            comment,
            self.ty.into_model()?,
            field_as,
        ))
    }
}

#[derive(Debug)]
pub struct File<'input> {
    pub options: Vec<Loc<OptionDecl<'input>>>,
    pub uses: Vec<Loc<UseDecl<'input>>>,
    pub decls: Vec<Loc<Decl<'input>>>,
}

impl<'input> IntoModel for File<'input> {
    type Output = RpFile;

    fn into_model(self) -> Result<RpFile> {
        let options = Options::new(self.options.into_model()?);

        let file = RpFile {
            options: options,
            uses: self.uses.into_model()?,
            decls: self.decls.into_model()?,
        };

        Ok(file)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum Name {
    Relative { parts: Vec<String> },
    Absolute {
        prefix: Option<String>,
        parts: Vec<String>,
    },
}

#[derive(Debug, PartialEq, Clone)]
pub struct Instance<'input> {
    pub name: Name,
    pub arguments: Loc<Vec<Loc<FieldInit<'input>>>>,
}

impl<'input> IntoModel for Instance<'input> {
    type Output = RpInstance;

    fn into_model(self) -> Result<RpInstance> {
        let instance = RpInstance {
            name: self.name.into_model()?,
            arguments: self.arguments.into_model()?,
        };

        Ok(instance)
    }
}

#[derive(Debug)]
pub struct InterfaceBody<'input> {
    pub name: &'input str,
    pub comment: Vec<&'input str>,
    pub members: Vec<Loc<Member<'input>>>,
    pub sub_types: Vec<Loc<SubType<'input>>>,
}

impl<'input> IntoModel for InterfaceBody<'input> {
    type Output = Rc<RpInterfaceBody>;

    fn into_model(self) -> Result<Rc<RpInterfaceBody>> {
        use std::collections::btree_map::Entry::*;

        let (fields, codes, options, match_decl) = members_into_model(self.members)?;

        let mut sub_types: BTreeMap<String, Loc<Rc<RpSubType>>> = BTreeMap::new();

        for sub_type in self.sub_types.into_model()? {
            // key has to be owned by entry
            let key = sub_type.name.clone();

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

        let interface_body = RpInterfaceBody {
            name: self.name.to_owned(),
            comment: self.comment.into_iter().map(ToOwned::to_owned).collect(),
            fields: fields,
            codes: codes,
            match_decl: match_decl,
            sub_types: sub_types,
        };

        Ok(Rc::new(interface_body))
    }
}

/// Adds a method for all types that supports conversion into core types.
pub trait IntoModel {
    type Output;

    /// Convert the current type to a model.
    fn into_model(self) -> Result<Self::Output>;
}

/// Generic implementation for vectors.
impl<T> IntoModel for Loc<T>
where
    T: IntoModel,
{
    type Output = Loc<T::Output>;

    fn into_model(self) -> Result<Self::Output> {
        let (value, pos) = self.both();
        Ok(Loc::new(value.into_model()?, pos))
    }
}

/// Generic implementation for vectors.
impl<T> IntoModel for Vec<T>
where
    T: IntoModel,
{
    type Output = Vec<T::Output>;

    fn into_model(self) -> Result<Self::Output> {
        let mut out = Vec::new();

        for v in self {
            out.push(v.into_model()?);
        }

        Ok(out)
    }
}

impl<T> IntoModel for Option<T>
where
    T: IntoModel,
{
    type Output = Option<T::Output>;

    fn into_model(self) -> Result<Self::Output> {
        if let Some(value) = self {
            return Ok(Some(value.into_model()?));
        }

        Ok(None)
    }
}

impl<T> IntoModel for Box<T>
where
    T: IntoModel,
{
    type Output = Box<T::Output>;

    fn into_model(self) -> Result<Self::Output> {
        Ok(Box::new((*self).into_model()?))
    }
}

impl<'a> IntoModel for &'a str {
    type Output = String;

    fn into_model(self) -> Result<Self::Output> {
        Ok(self.to_owned())
    }
}

impl IntoModel for String {
    type Output = String;

    fn into_model(self) -> Result<Self::Output> {
        Ok(self)
    }
}

impl IntoModel for RpPackage {
    type Output = RpPackage;

    fn into_model(self) -> Result<Self::Output> {
        Ok(self)
    }
}

impl IntoModel for Name {
    type Output = RpName;

    fn into_model(self) -> Result<Self::Output> {
        use self::Name::*;

        let out = match self {
            Relative { parts } => {
                RpName {
                    prefix: None,
                    parts: parts,
                }
            }
            Absolute { prefix, parts } => {
                RpName {
                    prefix: prefix,
                    parts: parts,
                }
            }
        };

        Ok(out)
    }
}

impl<'input> IntoModel for (&'input Path, usize, usize) {
    type Output = (PathBuf, usize, usize);

    fn into_model(self) -> Result<Self::Output> {
        Ok((self.0.to_owned(), self.1, self.2))
    }
}

#[derive(Debug)]
pub enum MatchCondition<'input> {
    /// Match a specific value.
    Value(Loc<Value<'input>>),
    /// Match a type, and add a binding for the given name that can be resolved in the action.
    Type(Loc<MatchVariable<'input>>),
}

impl<'input> IntoModel for MatchCondition<'input> {
    type Output = RpMatchCondition;

    fn into_model(self) -> Result<RpMatchCondition> {
        let match_condition = match self {
            MatchCondition::Value(value) => RpMatchCondition::Value(value.into_model()?),
            MatchCondition::Type(ty) => RpMatchCondition::Type(ty.into_model()?),
        };

        Ok(match_condition)
    }
}

#[derive(Debug)]
pub struct MatchMember<'input> {
    pub comment: Vec<&'input str>,
    pub condition: Loc<MatchCondition<'input>>,
    pub object: Loc<Object<'input>>,
}

impl<'input> IntoModel for MatchMember<'input> {
    type Output = RpMatchMember;

    fn into_model(self) -> Result<RpMatchMember> {
        let member = RpMatchMember {
            comment: self.comment.into_model()?,
            condition: self.condition.into_model()?,
            object: self.object.into_model()?,
        };

        Ok(member)
    }
}

#[derive(Debug)]
pub struct MatchVariable<'input> {
    pub name: &'input str,
    pub ty: Type,
}

impl<'input> IntoModel for MatchVariable<'input> {
    type Output = RpMatchVariable;

    fn into_model(self) -> Result<RpMatchVariable> {
        let match_variable = RpMatchVariable {
            name: self.name.into_model()?,
            ty: self.ty.into_model()?,
        };

        Ok(match_variable)
    }
}

#[derive(Debug)]
pub enum Member<'input> {
    Field(Field<'input>),
    Code(&'input str, Vec<String>),
    Option(OptionDecl<'input>),
    Match(MatchMember<'input>),
}

#[derive(Debug, PartialEq, Clone)]
pub enum Object<'input> {
    Instance(Loc<Instance<'input>>),
    Constant(Loc<Name>),
}

impl<'input> IntoModel for Object<'input> {
    type Output = RpObject;

    fn into_model(self) -> Result<RpObject> {
        use self::Object::*;

        let out = match self {
            Instance(instance) => RpObject::Instance(instance.into_model()?),
            Constant(constant) => RpObject::Constant(constant.into_model()?),
        };

        Ok(out)
    }
}

#[derive(Debug, Clone)]
pub struct OptionDecl<'input> {
    pub name: &'input str,
    pub values: Vec<Loc<Value<'input>>>,
}

impl<'input> IntoModel for OptionDecl<'input> {
    type Output = RpOptionDecl;

    fn into_model(self) -> Result<RpOptionDecl> {
        let decl = RpOptionDecl {
            name: self.name.to_owned(),
            values: self.values.into_model()?,
        };

        Ok(decl)
    }
}

#[derive(Debug)]
pub enum PathSegment<'input> {
    Literal { value: Loc<String> },
    Variable {
        name: Loc<&'input str>,
        ty: Loc<Type>,
    },
}

impl<'input> IntoModel for PathSegment<'input> {
    type Output = RpPathSegment;

    fn into_model(self) -> Result<RpPathSegment> {
        let out = match self {
            PathSegment::Literal { value } => RpPathSegment::Literal { value: value.into_model()? },
            PathSegment::Variable { name, ty } => {
                RpPathSegment::Variable {
                    name: name.into_model()?,
                    ty: ty.into_model()?,
                }
            }
        };

        Ok(out)
    }
}

#[derive(Debug)]
pub struct PathSpec<'input> {
    pub segments: Vec<PathSegment<'input>>,
}

impl<'input> IntoModel for PathSpec<'input> {
    type Output = RpPathSpec;

    fn into_model(self) -> Result<RpPathSpec> {
        Ok(RpPathSpec { segments: self.segments.into_model()? })
    }
}

#[derive(Debug)]
pub struct ServiceBody<'input> {
    pub name: &'input str,
    pub comment: Vec<&'input str>,
    pub children: Vec<ServiceNested<'input>>,
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
    comment: Vec<String>,
    status: Option<Loc<RpNumber>>,
    produces: Option<Loc<String>>,
    ty: Option<Loc<Type>>,
    options: Vec<Loc<OptionDecl>>,
) -> Result<RpServiceReturns> {
    let options = Options::new(options.into_model()?);

    let produces = produces.or(options.find_one_string("produces")?);

    let produces = if let Some(produces) = produces {
        let (produces, pos) = produces.both();

        let produces = produces.parse().chain_err(|| {
            ErrorKind::Pos("not a valid mime type".to_owned(), pos.into())
        })?;

        Some(produces)
    } else {
        None
    };

    let status = status.or(options.find_one_number("status")?);

    let status = if let Some(status) = status {
        let (status, pos) = status.both();

        let status = status.to_u32().ok_or_else(|| {
            ErrorKind::Pos("not a valid status".to_owned(), pos.into())
        })?;

        Some(status)
    } else {
        None
    };

    Ok(RpServiceReturns {
        comment: comment,
        ty: ty.into_model()?,
        produces: produces,
        status: status,
    })
}

fn convert_accepts(
    comment: Vec<String>,
    accepts: Option<Loc<String>>,
    ty: Option<Loc<Type>>,
    options: Vec<Loc<OptionDecl>>,
) -> Result<RpServiceAccepts> {
    let options = Options::new(options.into_model()?);

    let accepts = accepts.or(options.find_one_string("accept")?);

    let accepts = if let Some(accepts) = accepts {
        let (accepts, pos) = accepts.both();

        let accepts = accepts.parse().chain_err(|| {
            ErrorKind::Pos("not a valid mime type".to_owned(), pos.into())
        })?;

        Some(accepts)
    } else {
        None
    };

    Ok(RpServiceAccepts {
        comment: comment,
        ty: ty.into_model()?,
        accepts: accepts,
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
    type Output = Rc<RpServiceBody>;

    fn into_model(self) -> Result<Rc<RpServiceBody>> {
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
                process_child(&mut queue, &parent, child)?;
            }

            let p = parent.as_ref().try_borrow()?;

            if p.method.is_some() {
                endpoints.push(unwind(parent.clone())?);
            }
        }

        let endpoints = endpoints.into_iter().rev().collect();

        let service_body = RpServiceBody {
            name: self.name.to_owned(),
            comment: self.comment.into_iter().map(ToOwned::to_owned).collect(),
            endpoints: endpoints,
        };

        return Ok(Rc::new(service_body));

        fn process_child<'input>(
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
                        method: method.into_model()?,
                        path: path.into_model()?,
                        options: options.into_model()?,
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
                    let returns = convert_return(comment, status, produces, ty, options)?;
                    parent.try_borrow_mut()?.push_returns(returns);
                }
                ServiceNested::Accepts {
                    comment,
                    accepts,
                    ty,
                    options,
                } => {
                    let comment = comment.into_iter().map(ToOwned::to_owned).collect();
                    let accepts = convert_accepts(comment, accepts, ty, options)?;
                    parent.try_borrow_mut()?.push_accepts(accepts);
                }
            }

            Ok(())
        }
    }
}

#[derive(Debug)]
pub enum ServiceNested<'input> {
    Endpoint {
        method: Option<Loc<&'input str>>,
        path: Option<Loc<PathSpec<'input>>>,
        comment: Vec<&'input str>,
        options: Vec<Loc<OptionDecl<'input>>>,
        children: Vec<ServiceNested<'input>>,
    },
    Returns {
        comment: Vec<&'input str>,
        status: Option<Loc<RpNumber>>,
        produces: Option<Loc<String>>,
        ty: Option<Loc<Type>>,
        options: Vec<Loc<OptionDecl<'input>>>,
    },
    Accepts {
        comment: Vec<&'input str>,
        accepts: Option<Loc<String>>,
        ty: Option<Loc<Type>>,
        options: Vec<Loc<OptionDecl<'input>>>,
    },
}

impl<'input> ServiceNested<'input> {
    pub fn is_terminus(&self) -> bool {
        match *self {
            ServiceNested::Returns { .. } => true,
            ServiceNested::Accepts { .. } => true,
            _ => false,
        }
    }
}

/// Sub-types in interface declarations.
#[derive(Debug)]
pub struct SubType<'input> {
    pub name: &'input str,
    pub comment: Vec<&'input str>,
    pub members: Vec<Loc<Member<'input>>>,
}

impl<'input> IntoModel for SubType<'input> {
    type Output = Rc<RpSubType>;

    fn into_model(self) -> Result<Rc<RpSubType>> {
        let mut fields: Vec<Loc<RpField>> = Vec::new();
        let mut codes = Vec::new();
        let mut options = Vec::new();
        let mut match_decl = RpMatchDecl::new();

        for member in self.members {
            let (member, pos) = member.both();

            match member {
                Member::Field(field) => {
                    let field = field.into_model()?;

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
                Member::Code(context, lines) => {
                    codes.push(code(pos, context.to_owned(), lines));
                }
                Member::Option(option) => {
                    options.push(Loc::new(option.into_model()?, pos));
                }
                Member::Match(m) => {
                    match_decl.push(Loc::new(m.into_model()?, pos))?;
                }
            }
        }

        let options = Options::new(options);

        let names = options.find_all_strings("name")?;

        let sub_type = RpSubType {
            name: self.name.to_owned(),
            comment: self.comment.into_iter().map(ToOwned::to_owned).collect(),
            fields: fields,
            codes: codes,
            names: names,
            match_decl: match_decl,
        };

        Ok(Rc::new(sub_type))
    }
}

#[derive(Debug)]
pub struct TupleBody<'input> {
    pub name: &'input str,
    pub comment: Vec<&'input str>,
    pub members: Vec<Loc<Member<'input>>>,
}

impl<'input> IntoModel for TupleBody<'input> {
    type Output = Rc<RpTupleBody>;

    fn into_model(self) -> Result<Rc<RpTupleBody>> {
        let (fields, codes, options, match_decl) = members_into_model(self.members)?;

        let _options = Options::new(options);

        let tuple_body = RpTupleBody {
            name: self.name.to_owned(),
            comment: self.comment.into_iter().map(ToOwned::to_owned).collect(),
            fields: fields,
            codes: codes,
            match_decl: match_decl,
        };

        Ok(Rc::new(tuple_body))
    }
}

#[derive(Debug)]
pub struct TypeBody<'input> {
    pub name: &'input str,
    pub comment: Vec<&'input str>,
    pub members: Vec<Loc<Member<'input>>>,
}

impl<'input> IntoModel for TypeBody<'input> {
    type Output = Rc<RpTypeBody>;

    fn into_model(self) -> Result<Rc<RpTypeBody>> {
        let (fields, codes, options, match_decl) = members_into_model(self.members)?;

        let options = Options::new(options);

        let reserved: HashSet<Loc<String>> = options
            .find_all_identifiers("reserved")?
            .into_iter()
            .collect();

        let type_body = RpTypeBody {
            name: self.name.to_owned(),
            comment: self.comment.into_iter().map(ToOwned::to_owned).collect(),
            fields: fields,
            codes: codes,
            match_decl: match_decl,
            reserved: reserved,
        };

        Ok(Rc::new(type_body))
    }
}

#[derive(Debug)]
pub struct UseDecl<'input> {
    pub package: Loc<RpPackage>,
    pub version_req: Option<Loc<VersionReq>>,
    pub alias: Option<&'input str>,
}

impl<'input> IntoModel for UseDecl<'input> {
    type Output = RpUseDecl;

    fn into_model(self) -> Result<RpUseDecl> {
        let use_decl = RpUseDecl {
            package: self.package.into_model()?,
            version_req: self.version_req,
            alias: self.alias.map(ToOwned::to_owned),
        };

        Ok(use_decl)
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
    members: Vec<Loc<Member>>,
) -> Result<(Fields, Codes, OptionVec, RpMatchDecl)> {
    use self::Member::*;

    let mut fields: Vec<Loc<RpField>> = Vec::new();
    let mut codes = Vec::new();
    let mut options: Vec<Loc<RpOptionDecl>> = Vec::new();
    let mut match_decl = RpMatchDecl::new();

    for member in members {
        let (value, pos) = member.both();

        match value {
            Field(field) => {
                let field = field.into_model()?;

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
                options.push(Loc::new(option.into_model()?, pos));
            }
            Match(m) => {
                match_decl.push(Loc::new(m.into_model()?, pos))?;
            }
        }
    }

    Ok((fields, codes, options, match_decl))
}

/// Generate ordinal values.
pub struct OrdinalGenerator {
    next_ordinal: u32,
    ordinals: HashMap<u32, Pos>,
}

impl OrdinalGenerator {
    pub fn new() -> OrdinalGenerator {
        OrdinalGenerator {
            next_ordinal: 0,
            ordinals: HashMap::new(),
        }
    }

    pub fn next(&mut self, ordinal: &Option<Loc<Value>>) -> Result<u32> {
        if let Some(ref ordinal) = *ordinal {
            let pos = ordinal.pos();

            if let Value::Number(ref number) = *ordinal.as_ref() {
                let n: u32 = number.to_u32().ok_or_else(
                    || ErrorKind::Overflow(pos.into()),
                )?;

                if let Some(other) = self.ordinals.get(&n) {
                    return Err(
                        ErrorKind::Pos("duplicate ordinal".to_owned(), other.into()).into(),
                    );
                }

                self.ordinals.insert(n, pos.clone());
                self.next_ordinal = n + 1;
                return Ok(n);
            }

            return Err(
                ErrorKind::Pos("must be a number".to_owned(), pos.into()).into(),
            );
        }

        let o = self.next_ordinal;

        self.next_ordinal += 1;

        if let Some(other) = self.ordinals.get(&o) {
            return Err(
                ErrorKind::Pos(
                    format!("generated ordinal {} conflicts with existing", o),
                    other.into(),
                ).into(),
            );
        }

        Ok(o)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum Value<'input> {
    String(String),
    Number(RpNumber),
    Boolean(bool),
    Identifier(&'input str),
    Array(Vec<Loc<Value<'input>>>),
    Object(Loc<Object<'input>>),
}

impl<'input> IntoModel for Value<'input> {
    type Output = RpValue;

    fn into_model(self) -> Result<RpValue> {
        let out = match self {
            Value::String(string) => RpValue::String(string),
            Value::Number(number) => RpValue::Number(number),
            Value::Boolean(boolean) => RpValue::Boolean(boolean),
            Value::Identifier(identifier) => RpValue::Identifier(identifier.to_owned()),
            Value::Array(inner) => RpValue::Array(inner.into_model()?),
            Value::Object(object) => RpValue::Object(object.into_model()?),
        };

        Ok(out)
    }
}
