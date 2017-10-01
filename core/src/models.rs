//! Data Models for the final model stage stage.

use super::loc::Loc;
use super::mime::Mime;
use super::options::Options;
use super::rp_modifier::RpModifier;
use super::rp_number::RpNumber;
use super::rp_versioned_package::RpVersionedPackage;
use errors::*;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::fmt;
use std::rc::Rc;
use std::slice;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RpDecl {
    Type(Rc<Loc<RpTypeBody>>),
    Tuple(Rc<Loc<RpTupleBody>>),
    Interface(Rc<Loc<RpInterfaceBody>>),
    Enum(Rc<Loc<RpEnumBody>>),
    Service(Rc<Loc<RpServiceBody>>),
}

#[derive(Debug, Clone)]
pub enum RpRegistered {
    Type(Rc<Loc<RpTypeBody>>),
    Tuple(Rc<Loc<RpTupleBody>>),
    Interface(Rc<Loc<RpInterfaceBody>>),
    SubType(Rc<Loc<RpInterfaceBody>>, Rc<Loc<RpSubType>>),
    Enum(Rc<Loc<RpEnumBody>>),
    EnumVariant(Rc<Loc<RpEnumBody>>, Rc<Loc<RpEnumVariant>>),
    Service(Rc<Loc<RpServiceBody>>),
}

#[derive(Debug, Clone, Serialize)]
pub struct RpCode {
    pub context: String,
    pub lines: Vec<String>,
}

pub struct DeclIter<'a> {
    iter: slice::Iter<'a, Rc<Loc<RpDecl>>>,
}

impl<'a> Iterator for DeclIter<'a> {
    type Item = &'a Rc<Loc<RpDecl>>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl RpDecl {
    pub fn decls(&self) -> DeclIter {
        use self::RpDecl::*;

        let iter = match *self {
            Type(ref body) => body.decls.iter(),
            Interface(ref body) => body.decls.iter(),
            Enum(ref body) => body.decls.iter(),
            Tuple(ref body) => body.decls.iter(),
            Service(ref body) => body.decls.iter(),
        };

        DeclIter { iter: iter }
    }

    pub fn local_name(&self) -> &str {
        use self::RpDecl::*;

        match *self {
            Type(ref body) => body.local_name.as_str(),
            Interface(ref body) => body.local_name.as_str(),
            Enum(ref body) => body.local_name.as_str(),
            Tuple(ref body) => body.local_name.as_str(),
            Service(ref body) => body.local_name.as_str(),
        }
    }

    pub fn name(&self) -> &RpName {
        use self::RpDecl::*;

        match *self {
            Type(ref body) => &body.name,
            Interface(ref body) => &body.name,
            Enum(ref body) => &body.name,
            Tuple(ref body) => &body.name,
            Service(ref body) => &body.name,
        }
    }

    pub fn comment(&self) -> &[String] {
        use self::RpDecl::*;

        match *self {
            Type(ref body) => &body.comment,
            Interface(ref body) => &body.comment,
            Enum(ref body) => &body.comment,
            Tuple(ref body) => &body.comment,
            Service(ref body) => &body.comment,
        }
    }

    /// Convert a declaration into its registered types.
    pub fn into_registered_type(&self) -> Vec<RpRegistered> {
        use self::RpDecl::*;

        let mut out = Vec::new();

        match *self {
            Type(ref ty) => {
                out.push(RpRegistered::Type(ty.clone()));
            }
            Interface(ref interface) => {
                for sub_type in interface.sub_types.values() {
                    out.push(RpRegistered::SubType(interface.clone(), sub_type.clone()));

                    for d in &sub_type.decls {
                        out.extend(d.into_registered_type());
                    }
                }

                out.push(RpRegistered::Interface(interface.clone()));
            }
            Enum(ref en) => {
                for variant in &en.variants {
                    out.push(RpRegistered::EnumVariant(en.clone(), variant.clone()));
                }

                out.push(RpRegistered::Enum(en.clone()));
            }
            Tuple(ref tuple) => {
                out.push(RpRegistered::Tuple(tuple.clone()));
            }
            Service(ref service) => {
                out.push(RpRegistered::Service(service.clone()));
            }
        }

        for d in self.decls() {
            out.extend(d.into_registered_type());
        }

        out
    }
}

impl fmt::Display for RpDecl {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::RpDecl::*;

        match *self {
            Type(ref body) => write!(f, "type {}", body.name),
            Interface(ref body) => write!(f, "interface {}", body.name),
            Enum(ref body) => write!(f, "enum {}", body.name),
            Tuple(ref body) => write!(f, "tuple {}", body.name),
            Service(ref body) => write!(f, "service {}", body.name),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RpEnumBody {
    pub name: RpName,
    pub local_name: String,
    pub comment: Vec<String>,
    /// Inner declarations.
    pub decls: Vec<Rc<Loc<RpDecl>>>,
    pub variant_type: RpEnumType,
    pub variants: Vec<Rc<Loc<RpEnumVariant>>>,
    pub fields: Vec<Loc<RpField>>,
    pub codes: Vec<Loc<RpCode>>,
    pub serialized_as: Option<Loc<String>>,
    pub serialized_as_name: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RpEnumVariant {
    pub name: RpName,
    pub local_name: Loc<String>,
    pub comment: Vec<String>,
    pub ordinal: RpEnumOrdinal,
}

#[derive(Debug, Clone, Serialize)]
pub enum RpEnumType {
    String,
    Number,
}

impl RpEnumType {
    pub fn is_assignable_from(&self, value: &RpValue) -> bool {
        use self::RpEnumType::*;

        match (self, value) {
            (&Number, &RpValue::Number(_)) => true,
            (&String, &RpValue::String(_)) => true,
            _ => false,
        }
    }
}

impl fmt::Display for RpEnumType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::RpEnumType::*;

        match *self {
            String => write!(f, "string"),
            Number => write!(f, "number"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum RpEnumOrdinal {
    String(String),
    Number(u32),
}

#[derive(Debug, Clone, Serialize)]
pub struct RpFieldInit {
    pub name: Loc<String>,
    pub value: Loc<RpValue>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RpField {
    pub modifier: RpModifier,
    pub name: String,
    pub comment: Vec<String>,
    #[serde(rename = "type")]
    pub ty: RpType,
    pub field_as: Option<Loc<String>>,
}

impl RpField {
    pub fn new(
        modifier: RpModifier,
        name: String,
        comment: Vec<String>,
        ty: RpType,
        field_as: Option<Loc<String>>,
    ) -> RpField {
        RpField {
            modifier: modifier,
            name: name,
            comment: comment,
            ty: ty,
            field_as: field_as,
        }
    }

    pub fn is_optional(&self) -> bool {
        match self.modifier {
            RpModifier::Optional => true,
            _ => false,
        }
    }

    pub fn ident(&self) -> &str {
        &self.name
    }

    pub fn name(&self) -> &str {
        self.field_as.as_ref().map(Loc::value).unwrap_or(&self.name)
    }

    pub fn display(&self) -> String {
        self.name.to_owned()
    }
}

#[derive(Debug)]
pub struct RpFile {
    pub options: Options,
    pub decls: Vec<Loc<RpDecl>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RpInstance {
    pub name: RpName,
    pub arguments: Loc<Vec<Loc<RpFieldInit>>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RpInterfaceBody {
    pub name: RpName,
    pub local_name: String,
    pub comment: Vec<String>,
    /// Inner declarations.
    pub decls: Vec<Rc<Loc<RpDecl>>>,
    pub fields: Vec<Loc<RpField>>,
    pub codes: Vec<Loc<RpCode>>,
    pub sub_types: BTreeMap<String, Rc<Loc<RpSubType>>>,
}

impl RpInterfaceBody {
    pub fn fields<'a>(&'a self) -> Box<Iterator<Item = &Loc<RpField>> + 'a> {
        Box::new(self.fields.iter())
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RpName {
    /// Alias used if the name was imported from another package.
    pub prefix: Option<String>,
    /// Package that name belongs to.
    pub package: RpVersionedPackage,
    /// Absolute parts of the name, from the root of the package.
    pub parts: Vec<String>,
}

impl RpName {
    pub fn new(prefix: Option<String>, package: RpVersionedPackage, parts: Vec<String>) -> RpName {
        RpName {
            prefix: prefix,
            package: package,
            parts: parts,
        }
    }

    pub fn extend<I>(&self, it: I) -> RpName
    where
        I: IntoIterator<Item = String>,
    {
        let mut parts = self.parts.clone();
        parts.extend(it);

        RpName {
            prefix: self.prefix.clone(),
            package: self.package.clone(),
            parts: parts,
        }
    }

    pub fn push(&self, part: String) -> RpName {
        let mut parts = self.parts.clone();
        parts.push(part);

        RpName {
            prefix: self.prefix.clone(),
            package: self.package.clone(),
            parts: parts,
        }
    }

    pub fn join<S: AsRef<str>>(&self, joiner: S) -> String {
        self.parts.join(joiner.as_ref())
    }

    pub fn without_prefix(self) -> RpName {
        RpName {
            prefix: None,
            package: self.package,
            parts: self.parts,
        }
    }

    pub fn with_package(self, package: RpVersionedPackage) -> RpName {
        RpName {
            prefix: self.prefix,
            package: package,
            parts: self.parts,
        }
    }

    pub fn is_same(&self, other: &RpName) -> bool {
        self.package == other.package && self.parts == other.parts
    }
}

impl fmt::Display for RpName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(ref prefix) = self.prefix {
            write!(f, "{}::{}", prefix, self.parts.join("::"))
        } else {
            write!(f, "{}", self.parts.join("::"))
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum RpCreator {
    Instance(Loc<RpInstance>),
    Constant(Loc<RpName>),
}

#[derive(Debug, Clone, Serialize)]
pub struct RpOptionDecl {
    pub name: String,
    pub values: Vec<Loc<RpValue>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RpPathSegment {
    Literal { value: Loc<String> },
    Variable { name: Loc<String>, ty: Loc<RpType> },
}

impl RpPathSegment {
    pub fn path(&self) -> String {
        match *self {
            RpPathSegment::Literal { ref value } => value.value().to_owned(),
            RpPathSegment::Variable { ref name, .. } => format!("{{{}}}", name.value()),
        }
    }

    pub fn id(&self) -> &str {
        match *self {
            RpPathSegment::Literal { ref value } => value.value().as_str(),
            RpPathSegment::Variable { ref name, .. } => name.value().as_str(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RpPathSpec {
    pub segments: Vec<RpPathSegment>,
}

impl RpPathSpec {
    pub fn url(&self) -> String {
        let segments: Vec<String> = self.segments.iter().map(RpPathSegment::path).collect();
        format!("/{}", segments.join("/"))
    }

    pub fn id_fragments(&self) -> Vec<&str> {
        self.segments.iter().map(RpPathSegment::id).collect()
    }
}

impl RpRegistered {
    pub fn name(&self) -> &RpName {
        use self::RpRegistered::*;

        match *self {
            Type(ref target) => &target.name,
            Tuple(ref target) => &target.name,
            Service(ref target) => &target.name,
            Interface(ref target) => &target.name,
            Enum(ref target) => &target.name,
            SubType(_, ref target) => &target.name,
            EnumVariant(_, ref target) => &target.name,
        }
    }

    pub fn is_assignable_from(&self, other: &RpRegistered) -> bool {
        use self::RpRegistered::*;

        match (self, other) {
            // same type
            (&Type(ref s), &Type(ref o)) => Rc::ptr_eq(s, o),
            // tuple of same type
            (&Tuple(ref s), &Tuple(ref o)) => Rc::ptr_eq(s, o),
            // interface of same type
            (&Interface(ref s), &Interface(ref o)) => Rc::ptr_eq(s, o),
            // sub type of a given interfacce
            (&Interface(ref s), &SubType(ref o, _)) => Rc::ptr_eq(s, o),
            // exact sub type match
            (&SubType(_, ref s), &SubType(_, ref o)) => Rc::ptr_eq(s, o),
            // enum of same type
            (&Enum(ref s), &Enum(ref o)) => Rc::ptr_eq(s, o),
            // variant of a given enum
            (&Enum(ref s), &EnumVariant(ref o, _)) => Rc::ptr_eq(s, o),
            // exact variant match
            (&EnumVariant(_, ref s), &EnumVariant(_, ref o)) => Rc::ptr_eq(s, o),
            // same service
            (&Service(ref s), &Service(ref o)) => Rc::ptr_eq(s, o),
            _ => false,
        }
    }

    pub fn fields<'a>(&'a self) -> Result<Box<Iterator<Item = &Loc<RpField>> + 'a>> {
        use self::RpRegistered::*;

        let fields: Box<Iterator<Item = &Loc<RpField>>> = match *self {
            Type(ref target) => Box::new(target.fields.iter()),
            Tuple(ref target) => Box::new(target.fields.iter()),
            Interface(ref target) => Box::new(target.fields.iter()),
            Enum(ref target) => Box::new(target.fields.iter()),
            SubType(ref parent, ref target) => Box::new(
                parent.fields.iter().chain(target.fields.iter()),
            ),
            _ => {
                return Err(
                    format!("{}: type doesn't have fields", self.display()).into(),
                )
            }
        };

        Ok(fields)
    }

    pub fn display(&self) -> String {
        use self::RpRegistered::*;

        match *self {
            Type(ref body) => format!("type {}", body.name),
            Interface(ref body) => format!("interface {}", body.name),
            Enum(ref body) => format!("enum {}", body.name),
            Tuple(ref body) => format!("tuple {}", body.name),
            Service(ref body) => format!("service {}", body.name),
            SubType(_, ref sub_type) => format!("subtype {}", sub_type.name),
            EnumVariant(_, ref variant) => format!("variant {}", variant.name),
        }
    }

    pub fn local_name<PackageFn, InnerFn>(
        &self,
        name: &RpName,
        package_fn: PackageFn,
        inner_fn: InnerFn,
    ) -> String
    where
        PackageFn: Fn(Vec<&str>) -> String,
        InnerFn: Fn(Vec<&str>) -> String,
    {
        use self::RpRegistered::*;

        match *self {
            Type(_) | Interface(_) | Enum(_) | Tuple(_) | Service(_) => {
                let p = name.parts.iter().map(String::as_str).collect();
                package_fn(p)
            }
            SubType { .. } |
            EnumVariant { .. } => {
                let mut v: Vec<&str> = name.parts.iter().map(String::as_str).collect();
                let at = v.len().saturating_sub(2);
                let last = inner_fn(v.split_off(at));

                let mut parts = v.clone();
                parts.push(last.as_str());

                inner_fn(parts)
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RpServiceAccepts {
    pub comment: Vec<String>,
    pub ty: Option<Loc<RpType>>,
    pub accepts: Option<Mime>,
    pub alias: Option<Loc<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RpServiceBody {
    pub name: RpName,
    pub local_name: String,
    pub comment: Vec<String>,
    pub endpoints: Vec<RpServiceEndpoint>,
    pub decls: Vec<Rc<Loc<RpDecl>>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RpServiceEndpoint {
    pub method: Option<Loc<String>>,
    pub path: RpPathSpec,
    pub comment: Vec<String>,
    pub accepts: Vec<RpServiceAccepts>,
    pub returns: Vec<RpServiceReturns>,
}

impl RpServiceEndpoint {
    pub fn url(&self) -> String {
        self.path.url()
    }

    pub fn id_parts<F>(&self, filter: F) -> Vec<String>
    where
        F: Fn(&str) -> String,
    {
        let mut parts = Vec::new();

        if let Some(ref method) = self.method {
            parts.push(filter(method.value().as_str()));
        }

        parts.extend(self.path.id_fragments().into_iter().map(filter));
        parts
    }

    pub fn method(&self) -> Option<&str> {
        self.method.as_ref().map(|v| v.value().as_str())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RpServiceReturns {
    pub comment: Vec<String>,
    pub ty: Option<Loc<RpType>>,
    pub produces: Option<Mime>,
    pub status: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RpSubType {
    pub name: RpName,
    pub local_name: String,
    pub comment: Vec<String>,
    /// Inner declarations.
    pub decls: Vec<Rc<Loc<RpDecl>>>,
    pub fields: Vec<Loc<RpField>>,
    pub codes: Vec<Loc<RpCode>>,
    pub names: Vec<Loc<String>>,
}

impl RpSubType {
    pub fn name(&self) -> &str {
        self.names
            .iter()
            .map(|t| t.value().as_str())
            .nth(0)
            .unwrap_or(&self.local_name)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RpTupleBody {
    pub name: RpName,
    pub local_name: String,
    pub comment: Vec<String>,
    pub decls: Vec<Rc<Loc<RpDecl>>>,
    pub fields: Vec<Loc<RpField>>,
    pub codes: Vec<Loc<RpCode>>,
}

impl RpTupleBody {
    pub fn fields<'a>(&'a self) -> Box<Iterator<Item = &Loc<RpField>> + 'a> {
        Box::new(self.fields.iter())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RpTypeBody {
    pub name: RpName,
    pub local_name: String,
    pub comment: Vec<String>,
    pub decls: Vec<Rc<Loc<RpDecl>>>,
    pub fields: Vec<Loc<RpField>>,
    pub codes: Vec<Loc<RpCode>>,
    // Set of fields which are reserved for this type.
    pub reserved: HashSet<Loc<String>>,
}

impl RpTypeBody {
    pub fn verify(&self) -> Result<()> {
        for reserved in &self.reserved {
            if let Some(field) = self.fields.iter().find(|f| f.name() == reserved.value()) {
                return Err(
                    ErrorKind::ReservedField(field.pos().into(), reserved.pos().into()).into(),
                );
            }
        }

        Ok(())
    }

    pub fn fields<'a>(&'a self) -> Box<Iterator<Item = &Loc<RpField>> + 'a> {
        Box::new(self.fields.iter())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RpType {
    Double,
    Float,
    Signed { size: Option<usize> },
    Unsigned { size: Option<usize> },
    Boolean,
    String,
    Bytes,
    Any,
    Name { name: RpName },
    Array { inner: Box<RpType> },
    Map {
        key: Box<RpType>,
        value: Box<RpType>,
    },
}

impl fmt::Display for RpType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::RpType::*;

        match *self {
            Double => write!(f, "double"),
            Float => write!(f, "float"),
            Signed { ref size } => {
                if let Some(size) = *size {
                    write!(f, "signed/{}", size)
                } else {
                    write!(f, "signed")
                }
            }
            Unsigned { ref size } => {
                if let Some(size) = *size {
                    write!(f, "unsigned/{}", size)
                } else {
                    write!(f, "unsigned")
                }
            }
            Boolean => write!(f, "boolean"),
            String => write!(f, "string"),
            Name { ref name } => write!(f, "{}", name),
            Array { ref inner } => write!(f, "[{}]", inner),
            Map { ref key, ref value } => write!(f, "{{{}: {}}}", key, value),
            Any => write!(f, "any"),
            Bytes => write!(f, "bytes"),
        }
    }
}

impl RpType {
    /// Check if a given value is assignable to current type.
    pub fn is_assignable_from(&self, value: &RpValue) -> bool {
        use self::RpType::*;

        match (self, value) {
            (&Double, &RpValue::Number(_)) => true,
            (&Float, &RpValue::Number(_)) => true,
            (&Signed { .. }, &RpValue::Number(_)) => true,
            (&Unsigned { .. }, &RpValue::Number(_)) => true,
            (&Boolean, &RpValue::Boolean(_)) => true,
            (&String, &RpValue::String(_)) => true,
            (&Array { .. }, &RpValue::Array(_)) => true,
            (&Name { name: ref expected, .. }, &RpValue::Creator(ref creator)) => {
                match creator.value() {
                    &RpCreator::Constant(ref name) => expected.is_same(&*name),
                    &RpCreator::Instance(ref instance) => expected.is_same(&instance.name),
                }
            }
            _ => false,
        }
    }

    /// Convert to an enum variant type.
    pub fn as_enum_type(&self) -> Option<RpEnumType> {
        use self::RpType::*;

        match *self {
            Double | Float | Signed { .. } | Unsigned { .. } => Some(RpEnumType::Number),
            String => Some(RpEnumType::String),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum RpValue {
    String(String),
    Number(RpNumber),
    Boolean(bool),
    Identifier(String),
    Array(Vec<Loc<RpValue>>),
    Creator(Loc<RpCreator>),
}

impl RpValue {
    pub fn as_str(&self) -> Result<&str> {
        use self::RpValue::*;

        match *self {
            String(ref string) => Ok(string),
            _ => Err("not a string".into()),
        }
    }

    pub fn as_identifier(&self) -> Result<&str> {
        use self::RpValue::*;

        match *self {
            String(ref string) => Ok(string),
            Identifier(ref identifier) => Ok(identifier),
            _ => Err("unsupported identifier kind".into()),
        }
    }

    pub fn to_ordinal(self) -> Result<RpEnumOrdinal> {
        let ordinal = match self {
            RpValue::String(value) => RpEnumOrdinal::String(value),
            RpValue::Number(number) => {
                let number = number.to_u32().ok_or_else(|| ErrorKind::InvalidOrdinal)?;
                RpEnumOrdinal::Number(number)
            }
            _ => return Err(ErrorKind::InvalidOrdinal.into()),
        };

        Ok(ordinal)
    }
}

impl fmt::Display for RpValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let out = match *self {
            RpValue::String(_) => "<string>",
            RpValue::Number(_) => "<number>",
            RpValue::Boolean(_) => "<boolean>",
            RpValue::Identifier(_) => "<identifier>",
            RpValue::Array(_) => "<array>",
            RpValue::Creator(_) => "<creator>",
        };

        write!(f, "{}", out)
    }
}
