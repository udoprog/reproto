#![recursion_limit = "1000"]

extern crate mime;
extern crate num;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate error_chain;
pub extern crate semver;

pub mod object;
mod error_pos;
mod loc;
mod pos;
mod merge;
mod options;
pub mod errors;


pub use self::error_pos::*;
pub use self::loc::*;
pub use self::merge::*;
pub use self::options::*;
pub use self::pos::*;
use errors::*;
use num::bigint::BigInt;
use num::integer::Integer;
use num::traits::Signed;
use num::traits::cast::ToPrimitive;
pub use semver::Version;
pub use semver::VersionReq;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::fmt;
use std::rc::Rc;
use std::result;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct Mime(mime::Mime);

impl serde::Serialize for Mime {
    fn serialize<S>(&self, serializer: S) -> result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&format!("{}", self.0))
    }
}

impl FromStr for Mime {
    type Err = errors::Error;

    fn from_str(s: &str) -> errors::Result<Self> {
        Ok(Mime(
            s.parse().map_err(errors::ErrorKind::MimeFromStrError)?,
        ))
    }
}

impl fmt::Display for Mime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RpByTypeMatch {
    pub variable: Loc<RpMatchVariable>,
    pub object: Loc<RpObject>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RpByValueMatch {
    pub object: Loc<RpObject>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RpCode {
    pub context: String,
    pub lines: Vec<String>,
}

impl Merge for Vec<Loc<RpCode>> {
    fn merge(&mut self, source: Vec<Loc<RpCode>>) -> Result<()> {
        self.extend(source);
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RpDecl {
    Type(Rc<RpTypeBody>),
    Interface(Rc<RpInterfaceBody>),
    Enum(Rc<RpEnumBody>),
    Tuple(Rc<RpTupleBody>),
    Service(Rc<RpServiceBody>),
}

impl RpDecl {
    pub fn decls(&self) -> Vec<&Rc<Loc<RpDecl>>> {
        use self::RpDecl::*;

        match *self {
            Type(ref body) => body.decls.iter().collect(),
            Interface(ref body) => body.decls.iter().collect(),
            Enum(ref body) => body.decls.iter().collect(),
            Tuple(ref body) => body.decls.iter().collect(),
            Service(_) => vec![],
        }
    }

    pub fn name(&self) -> &str {
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
    pub fn into_registered_type(
        &self,
        type_id: &RpTypeId,
        pos: &Pos,
    ) -> Vec<(RpTypeId, Loc<RpRegistered>)> {
        use self::RpDecl::*;

        let mut out = Vec::new();

        match *self {
            Type(ref ty) => {
                let token = Loc::new(RpRegistered::Type(ty.clone()), pos.clone());
                out.push((type_id.clone(), token));
            }
            Interface(ref interface) => {
                let token = Loc::new(RpRegistered::Interface(interface.clone()), pos.clone());

                for (name, sub_type) in &interface.sub_types {
                    let type_id = type_id.extend(name.to_owned());

                    let token = Loc::new(
                        RpRegistered::SubType {
                            parent: interface.clone(),
                            sub_type: sub_type.as_ref().clone(),
                        },
                        pos.clone(),
                    );

                    out.push((type_id.clone(), token));

                    for d in &sub_type.decls {
                        let (d, pos) = d.ref_both();

                        out.extend(d.into_registered_type(
                            &type_id.extend(d.name().to_owned()),
                            pos,
                        ));
                    }
                }

                out.push((type_id.clone(), token));
            }
            Enum(ref en) => {
                let token = Loc::new(RpRegistered::Enum(en.clone()), pos.clone());

                for variant in &en.variants {
                    let token = Loc::new(
                        RpRegistered::EnumConstant {
                            parent: en.clone(),
                            variant: variant.as_ref().clone(),
                        },
                        pos.clone(),
                    );

                    let type_id = type_id.extend(variant.as_ref().name.as_ref().to_owned());
                    out.push((type_id, token));
                }

                out.push((type_id.clone(), token));
            }
            Tuple(ref tuple) => {
                let token = Loc::new(RpRegistered::Tuple(tuple.clone()), pos.clone());
                out.push((type_id.clone(), token));
            }
            Service(ref service) => {
                let token = Loc::new(RpRegistered::Service(service.clone()), pos.clone());
                out.push((type_id.clone(), token));
            }
        }

        for d in &self.decls() {
            let (d, pos) = d.ref_both();

            out.extend(d.into_registered_type(
                &type_id.extend(d.name().to_owned()),
                pos,
            ));
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

impl Merge for Loc<RpDecl> {
    fn merge(&mut self, source: Loc<RpDecl>) -> Result<()> {
        use self::RpDecl::*;

        let dest_pos = self.pos().clone();
        let m = self.as_mut();

        match *m {
            Type(ref mut body) => {
                if let Type(ref other) = *source {
                    return body.merge(other.clone());
                }
            }
            Enum(ref mut body) => {
                if let Enum(ref other) = *source {
                    if let Some(variant) = other.variants.iter().next() {
                        return Err(
                            ErrorKind::ExtendEnum(
                                "cannot extend enum with additional variants".to_owned(),
                                variant.pos().into(),
                                dest_pos.into(),
                            ).into(),
                        );
                    }

                    if let Some(field) = other.fields.iter().next() {
                        return Err(
                            ErrorKind::ExtendEnum(
                                "cannot extend enum with additional fields".to_owned(),
                                field.pos().into(),
                                dest_pos.into(),
                            ).into(),
                        );
                    }


                    return body.merge(other.clone());
                }
            }
            Interface(ref mut body) => {
                if let Interface(ref other) = *source {
                    return body.merge(other.clone());
                }
            }
            Tuple(ref mut body) => {
                if let Tuple(ref other) = *source {
                    return body.merge(other.clone());
                }
            }
            Service(ref mut body) => {
                if let Service(ref other) = *source {
                    return body.merge(other.clone());
                }
            }
        }

        return Err(
            ErrorKind::DeclMerge(
                format!("cannot merge with {}", source),
                source.pos().into(),
                dest_pos.into(),
            ).into(),
        );
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RpEnumBody {
    pub name: String,
    pub comment: Vec<String>,
    /// Inner declarations.
    pub decls: Vec<Rc<Loc<RpDecl>>>,
    pub variants: Vec<Loc<Rc<RpEnumVariant>>>,
    pub fields: Vec<Loc<RpField>>,
    pub codes: Vec<Loc<RpCode>>,
    pub match_decl: RpMatchDecl,
    pub serialized_as: Option<Loc<String>>,
    pub serialized_as_name: bool,
}

impl Merge for RpEnumBody {
    fn merge(&mut self, source: RpEnumBody) -> Result<()> {
        self.codes.merge(source.codes)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RpEnumVariant {
    pub name: Loc<String>,
    pub comment: Vec<String>,
    pub arguments: Vec<Loc<RpValue>>,
    pub ordinal: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct RpFieldInit {
    pub name: Loc<String>,
    pub value: Loc<RpValue>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RpField {
    pub modifier: RpModifier,
    name: String,
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
        self.field_as.as_ref().map(AsRef::as_ref).unwrap_or(
            &self.name,
        )
    }

    pub fn display(&self) -> String {
        self.name.to_owned()
    }
}

impl Merge for Vec<Loc<RpField>> {
    fn merge(&mut self, source: Vec<Loc<RpField>>) -> Result<()> {
        for f in source {
            if let Some(field) = self.iter().find(|e| e.name == f.name) {
                return Err(
                    ErrorKind::FieldConflict(f.name.clone(), f.pos().into(), field.pos().into())
                        .into(),
                );
            }

            self.push(f);
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct RpFile {
    pub options: Options,
    pub uses: Vec<Loc<RpUseDecl>>,
    pub decls: Vec<Loc<RpDecl>>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct RpInstance {
    pub name: RpName,
    pub arguments: Loc<Vec<Loc<RpFieldInit>>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RpInterfaceBody {
    pub name: String,
    pub comment: Vec<String>,
    /// Inner declarations.
    pub decls: Vec<Rc<Loc<RpDecl>>>,
    pub fields: Vec<Loc<RpField>>,
    pub codes: Vec<Loc<RpCode>>,
    pub match_decl: RpMatchDecl,
    pub sub_types: BTreeMap<String, Loc<Rc<RpSubType>>>,
}

impl RpInterfaceBody {
    pub fn fields<'a>(&'a self) -> Box<Iterator<Item = &Loc<RpField>> + 'a> {
        Box::new(self.fields.iter())
    }
}

impl Merge for RpInterfaceBody {
    fn merge(&mut self, source: RpInterfaceBody) -> Result<()> {
        self.fields.merge(source.fields)?;
        self.codes.merge(source.codes)?;
        self.sub_types.merge(source.sub_types)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RpMatchCondition {
    /// Match a specific value.
    Value(Loc<RpValue>),
    /// Match a type, and add a binding for the given name that can be resolved in the action.
    Type(Loc<RpMatchVariable>),
}

#[derive(Debug, Clone, Serialize)]
pub struct RpMatchDecl {
    pub by_value: Vec<(Loc<RpValue>, RpByValueMatch)>,
    pub by_type: Vec<(RpMatchKind, RpByTypeMatch)>,
}

impl RpMatchDecl {
    pub fn new() -> RpMatchDecl {
        RpMatchDecl {
            by_value: Vec::new(),
            by_type: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.by_value.is_empty() && self.by_type.is_empty()
    }

    pub fn identify_match_kind(&self, variable: &RpMatchVariable) -> RpMatchKind {
        match variable.ty {
            RpType::Double |
            RpType::Float |
            RpType::Signed { size: _ } |
            RpType::Unsigned { size: _ } => RpMatchKind::Number,
            RpType::Boolean => RpMatchKind::Boolean,
            RpType::String | RpType::Bytes => RpMatchKind::String,
            RpType::Any => RpMatchKind::Any,
            RpType::Name { name: _ } |
            RpType::Map { key: _, value: _ } => RpMatchKind::Object,
            RpType::Array { inner: _ } => RpMatchKind::Array,
        }
    }

    pub fn push(&mut self, member: Loc<RpMatchMember>) -> Result<()> {
        match *member.condition {
            RpMatchCondition::Type(ref variable) => {
                let match_kind = self.identify_match_kind(variable);

                {
                    // conflicting when type matches
                    let result = self.by_type.iter().find(|e| {
                        e.0 == match_kind || e.0 == RpMatchKind::Any
                    });

                    if let Some(&(_, ref existing_value)) = result {
                        let err = ErrorKind::MatchConflict(
                            member.condition.pos().into(),
                            existing_value.object.pos().into(),
                        );
                        return Err(err.into());
                    }
                }

                self.by_type.push((
                    match_kind,
                    RpByTypeMatch {
                        variable: variable.clone(),
                        object: member.object.clone(),
                    },
                ));
            }
            RpMatchCondition::Value(ref value) => {
                {
                    // conflicting when value matches
                    let result = self.by_value.iter().find(
                        |e| e.0.as_ref() == value.as_ref(),
                    );

                    if let Some(&(_, ref existing_value)) = result {
                        let err = ErrorKind::MatchConflict(
                            member.condition.pos().into(),
                            existing_value.object.pos().into(),
                        );
                        return Err(err.into());
                    }
                }

                self.by_value.push((
                    value.clone(),
                    RpByValueMatch { object: member.object.clone() },
                ));
            }
        }

        Ok(())
    }
}
/// Simplified types that _can_ be uniquely matched over for JSON.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RpMatchKind {
    Any,
    Object,
    Array,
    String,
    Boolean,
    Number,
}

#[derive(Debug, Clone, Serialize)]
pub struct RpMatchMember {
    pub comment: Vec<String>,
    pub condition: Loc<RpMatchCondition>,
    pub object: Loc<RpObject>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RpMatchVariable {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: RpType,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RpModifier {
    Required,
    Optional,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct RpName {
    pub prefix: Option<String>,
    pub parts: Vec<String>,
}

impl RpName {
    pub fn without_prefix(&self) -> RpName {
        RpName {
            prefix: None,
            parts: self.parts.clone(),
        }
    }

    pub fn with_parts(parts: Vec<String>) -> RpName {
        RpName {
            prefix: None,
            parts: parts,
        }
    }

    pub fn extend(&self, part: String) -> RpName {
        let mut parts = self.parts.clone();
        parts.push(part);

        RpName {
            prefix: self.prefix.clone(),
            parts: parts,
        }
    }

    pub fn join<S: AsRef<str>>(&self, joiner: S) -> String {
        self.parts.join(joiner.as_ref())
    }
}

impl fmt::Display for RpName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(ref prefix) = self.prefix {
            write!(f, "{}::", prefix)?;
        }

        write!(f, "{}", self.parts.join("."))
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RpNumber {
    // base digits
    pub digits: BigInt,
    // where the decimal point is
    pub decimal: usize,
}

impl RpNumber {
    fn multiple(&self) -> BigInt {
        let mut multiple: BigInt = 1.into();

        for _ in 0..self.decimal {
            let ten: BigInt = 10.into();
            multiple = multiple * ten;
        }

        multiple
    }

    pub fn to_u64(&self) -> Option<u64> {
        let m = self.multiple();

        self.digits.checked_div(&m).and_then(|r| r.to_u64())
    }

    pub fn to_u32(&self) -> Option<u32> {
        self.to_u64().map(|v| v as u32)
    }

    pub fn to_usize(&self) -> Option<usize> {
        self.to_u64().map(|v| v as usize)
    }

    pub fn to_f64(&self) -> Option<f64> {
        let multiple = self.multiple();
        let (base, decimal) = self.digits.div_mod_floor(&multiple);

        base.to_f64().and_then(|base| {
            decimal.to_f64().and_then(|decimal| {
                multiple.to_f64().map(
                    |multiple| base + (decimal / multiple),
                )
            })
        })
    }
}

impl From<u32> for RpNumber {
    fn from(value: u32) -> RpNumber {
        RpNumber {
            digits: value.into(),
            decimal: 0usize,
        }
    }
}

impl From<i32> for RpNumber {
    fn from(value: i32) -> RpNumber {
        RpNumber {
            digits: value.into(),
            decimal: 0usize,
        }
    }
}

impl fmt::Debug for RpNumber {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "RpNumber({})", self)
    }
}

impl fmt::Display for RpNumber {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.decimal == 0 {
            return write!(f, "{}", self.digits);
        }

        let multiple = self.multiple();
        let (base, decimal) = self.digits.abs().div_mod_floor(&multiple);

        let decimal = format!("{}", decimal);

        // pad leading zeros if needed
        let decimal = if decimal.len() < self.decimal {
            let mut s = String::new();

            for _ in decimal.len()..self.decimal {
                s.push('0');
            }

            s.push_str(&decimal);
            s
        } else {
            decimal
        };

        if self.digits.is_negative() {
            write!(f, "-{}.{}", base, decimal)
        } else {
            write!(f, "{}.{}", base, decimal)
        }
    }
}

impl serde::Serialize for RpNumber {
    fn serialize<S>(&self, serializer: S) -> result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let n = self.to_f64().unwrap();
        serializer.serialize_f64(n)
    }
}

#[cfg(test)]
mod test_numbers {
    use super::*;

    #[test]
    fn test_number() {
        let n = RpNumber {
            digits: 104321.into(),
            decimal: 2,
        };

        assert_eq!(Some(1043), n.to_u64());
        assert_eq!(Some(1043.21), n.to_f64());
    }

    #[test]
    fn test_negative() {
        let n = RpNumber {
            digits: (-104321).into(),
            decimal: 2,
        };

        assert_eq!(None, n.to_u64());
        assert_eq!(Some(-1043.21), n.to_f64());
    }

    #[test]
    fn test_display() {
        let n = RpNumber {
            digits: (104321).into(),
            decimal: 2,
        };

        assert_eq!("1043.21", format!("{}", n));

        let n2 = RpNumber {
            digits: (104321).into(),
            decimal: 0,
        };

        assert_eq!("104321", format!("{}", n2));

        let n3 = RpNumber {
            digits: (104321).into(),
            decimal: 10,
        };

        assert_eq!("0.0000104321", format!("{}", n3));
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum RpObject {
    Instance(Loc<RpInstance>),
    Constant(Loc<RpName>),
}

#[derive(Debug, Clone, Serialize)]
pub struct RpOptionDecl {
    pub name: String,
    pub values: Vec<Loc<RpValue>>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct RpPackage {
    pub parts: Vec<String>,
}

impl RpPackage {
    pub fn new(parts: Vec<String>) -> RpPackage {
        RpPackage { parts: parts }
    }

    pub fn join_versioned(&self, other: &RpVersionedPackage) -> RpVersionedPackage {
        let mut parts = self.parts.clone();

        if let Some(ref package) = other.package {
            parts.extend(package.parts.clone());
        }

        RpVersionedPackage::new(Some(RpPackage::new(parts)), other.version.clone())
    }

    pub fn join(&self, other: &RpPackage) -> RpPackage {
        let mut parts = self.parts.clone();
        parts.extend(other.parts.clone());
        RpPackage::new(parts)
    }

    pub fn into_type_id(&self, version: Option<Version>, name: RpName) -> RpTypeId {
        RpTypeId::new(RpVersionedPackage::new(Some(self.clone()), version), name)
    }
}

impl fmt::Display for RpPackage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.parts.join("."))
    }
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
            RpPathSegment::Literal { ref value } => value.as_ref().to_owned(),
            RpPathSegment::Variable { ref name, .. } => format!("{{{}}}", name.as_ref()),
        }
    }

    pub fn id(&self) -> &str {
        match *self {
            RpPathSegment::Literal { ref value } => value.as_ref().as_ref(),
            RpPathSegment::Variable { ref name, .. } => name.as_ref().as_ref(),
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

#[derive(Debug, Clone)]
pub enum RpRegistered {
    Type(Rc<RpTypeBody>),
    Interface(Rc<RpInterfaceBody>),
    Enum(Rc<RpEnumBody>),
    Tuple(Rc<RpTupleBody>),
    SubType {
        parent: Rc<RpInterfaceBody>,
        sub_type: Rc<RpSubType>,
    },
    EnumConstant {
        parent: Rc<RpEnumBody>,
        variant: Rc<RpEnumVariant>,
    },
    Service(Rc<RpServiceBody>),
}

impl RpRegistered {
    pub fn fields<'a>(&'a self) -> Result<Box<Iterator<Item = &Loc<RpField>> + 'a>> {
        use self::RpRegistered::*;

        let it: Box<Iterator<Item = &Loc<RpField>>> = match *self {
            Type(ref body) => Box::new(body.fields.iter()),
            Tuple(ref body) => Box::new(body.fields.iter()),
            SubType {
                ref parent,
                ref sub_type,
            } => Box::new(parent.fields.iter().chain(sub_type.fields.iter())),
            _ => {
                return Err("has no fields".into());
            }
        };

        Ok(it)
    }

    pub fn field_by_ident(&self, ident: &str) -> Result<Option<&Loc<RpField>>> {
        for field in self.fields()? {
            if field.ident() == ident {
                return Ok(Some(field));
            }
        }

        Ok(None)
    }

    pub fn is_assignable_from(&self, other: &RpRegistered) -> bool {
        use self::RpRegistered::*;

        match (self, other) {
            // exact type
            (&Type(ref target), &Type(ref source)) => Rc::ptr_eq(target, source),
            // exact tuple
            (&Tuple(ref target), &Tuple(ref source)) => Rc::ptr_eq(target, source),
            // exact service
            (&Service(ref target), &Service(ref source)) => Rc::ptr_eq(target, source),
            // exact interface, with unknown sub-type.
            (&Interface(ref target), &Interface(ref source)) => Rc::ptr_eq(target, source),
            // exact enum, with unknown value
            (&Enum(ref target), &Enum(ref source)) => Rc::ptr_eq(target, source),
            // sub-type to parent
            (&Interface(ref target),
             &SubType {
                 parent: ref source,
                 sub_type: _,
             }) => Rc::ptr_eq(target, source),
            // enum constant to parent type
            (&Enum(ref target),
             &EnumConstant {
                 parent: ref source,
                 variant: _,
             }) => Rc::ptr_eq(target, source),
            // exact matching sub-type
            (&SubType {
                 parent: ref target_parent,
                 sub_type: ref target,
             },
             &SubType {
                 parent: ref source_parent,
                 sub_type: ref source,
             }) => Rc::ptr_eq(target_parent, source_parent) && Rc::ptr_eq(target, source),
            // exact matching constant
            (&EnumConstant {
                 parent: ref target_parent,
                 variant: ref target,
             },
             &EnumConstant {
                 parent: ref source_parent,
                 variant: ref source,
             }) => Rc::ptr_eq(target_parent, source_parent) && Rc::ptr_eq(target, source),
            _ => false,
        }
    }

    pub fn display(&self) -> String {
        use self::RpRegistered::*;

        match *self {
            Type(ref body) => format!("type {}", body.name.to_owned()),
            Interface(ref body) => format!("interface {}", body.name.to_owned()),
            Enum(ref body) => format!("enum {}", body.name.to_owned()),
            Tuple(ref body) => format!("tuple {}", body.name.to_owned()),
            Service(ref body) => format!("service {}", body.name.to_owned()),
            SubType {
                ref parent,
                ref sub_type,
            } => format!("type {}.{}", parent.name, sub_type.name),
            EnumConstant {
                ref parent,
                ref variant,
            } => format!("{}.{}", parent.name, *variant.name),
        }
    }

    pub fn name(&self) -> Vec<&str> {
        use self::RpRegistered::*;

        match *self {
            Type(ref body) => vec![&body.name],
            Interface(ref body) => vec![&body.name],
            Enum(ref body) => vec![&body.name],
            Tuple(ref body) => vec![&body.name],
            Service(ref body) => vec![&body.name],
            SubType {
                ref parent,
                ref sub_type,
            } => vec![&parent.name, &sub_type.name],
            EnumConstant {
                ref parent,
                ref variant,
            } => vec![&parent.name, &variant.name],
        }
    }

    pub fn local_name<PackageFn, InnerFn>(
        &self,
        type_id: &RpTypeId,
        package_fn: PackageFn,
        inner_fn: InnerFn,
    ) -> String
    where
        PackageFn: Fn(Vec<&str>) -> String,
        InnerFn: Fn(Vec<&str>) -> String,
    {
        use self::RpRegistered::*;

        let name = &type_id.name;

        match *self {
            Type(_) | Interface(_) | Enum(_) | Tuple(_) | Service(_) => {
                let p = name.parts.iter().map(String::as_str).collect();
                package_fn(p)
            }
            SubType { .. } |
            EnumConstant { .. } => {
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

#[derive(Debug, Clone, PartialEq)]
pub struct RpRequiredPackage {
    pub package: RpPackage,
    pub version_req: Option<VersionReq>,
}

impl RpRequiredPackage {
    pub fn new(package: RpPackage, version_req: Option<VersionReq>) -> RpRequiredPackage {
        RpRequiredPackage {
            package: package,
            version_req: version_req,
        }
    }
}

impl fmt::Display for RpRequiredPackage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.package)?;

        if let Some(ref version_req) = self.version_req {
            write!(f, "@{}", version_req)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RpServiceAccepts {
    pub comment: Vec<String>,
    pub ty: Option<Loc<RpType>>,
    pub accepts: Option<Mime>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RpServiceBody {
    pub name: String,
    pub comment: Vec<String>,
    pub endpoints: Vec<RpServiceEndpoint>,
}

impl Merge for RpServiceBody {
    fn merge(&mut self, _source: RpServiceBody) -> Result<()> {
        Ok(())
    }
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
            parts.push(filter(method.as_ref().as_ref()));
        }

        parts.extend(self.path.id_fragments().into_iter().map(filter));
        parts
    }

    pub fn method(&self) -> Option<&str> {
        self.method.as_ref().map(|v| v.as_ref().as_ref())
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
    pub name: String,
    pub comment: Vec<String>,
    /// Inner declarations.
    pub decls: Vec<Rc<Loc<RpDecl>>>,
    pub fields: Vec<Loc<RpField>>,
    pub codes: Vec<Loc<RpCode>>,
    pub names: Vec<Loc<String>>,
    pub match_decl: RpMatchDecl,
}

impl RpSubType {
    pub fn name(&self) -> &str {
        self.names
            .iter()
            .map(|t| t.as_ref().as_str())
            .nth(0)
            .unwrap_or(&self.name)
    }

    pub fn fields<'a>(&'a self) -> Box<Iterator<Item = &Loc<RpField>> + 'a> {
        Box::new(self.fields.iter())
    }
}

impl Merge for RpSubType {
    fn merge(&mut self, source: RpSubType) -> Result<()> {
        self.fields.merge(source.fields)?;
        self.codes.merge(source.codes)?;
        self.names.extend(source.names);
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RpTupleBody {
    pub name: String,
    pub comment: Vec<String>,
    pub decls: Vec<Rc<Loc<RpDecl>>>,
    pub fields: Vec<Loc<RpField>>,
    pub codes: Vec<Loc<RpCode>>,
    pub match_decl: RpMatchDecl,
}

impl RpTupleBody {
    pub fn fields<'a>(&'a self) -> Box<Iterator<Item = &Loc<RpField>> + 'a> {
        Box::new(self.fields.iter())
    }
}

impl Merge for RpTupleBody {
    fn merge(&mut self, source: RpTupleBody) -> Result<()> {
        self.fields.merge(source.fields)?;
        self.codes.merge(source.codes)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RpTypeBody {
    pub name: String,
    pub comment: Vec<String>,
    pub decls: Vec<Rc<Loc<RpDecl>>>,
    pub fields: Vec<Loc<RpField>>,
    pub codes: Vec<Loc<RpCode>>,
    pub match_decl: RpMatchDecl,
    // Set of fields which are reserved for this type.
    pub reserved: HashSet<Loc<String>>,
}

impl RpTypeBody {
    pub fn verify(&self) -> Result<()> {
        for reserved in &self.reserved {
            if let Some(field) = self.fields.iter().find(|f| f.name() == reserved.as_ref()) {
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

impl Merge for RpTypeBody {
    fn merge(&mut self, source: RpTypeBody) -> Result<()> {
        self.fields.merge(source.fields)?;
        self.codes.merge(source.codes)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct RpTypeId {
    pub package: RpVersionedPackage,
    pub name: RpName,
}

impl RpTypeId {
    pub fn new(package: RpVersionedPackage, name: RpName) -> RpTypeId {
        RpTypeId {
            package: package,
            name: name,
        }
    }

    pub fn with_name(&self, name: RpName) -> RpTypeId {
        RpTypeId {
            package: self.package.clone(),
            name: name,
        }
    }

    pub fn extend(&self, part: String) -> RpTypeId {
        RpTypeId {
            package: self.package.clone(),
            name: self.name.extend(part),
        }
    }
}

impl fmt::Display for RpTypeId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[{}]::{}", self.package, self.name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
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
        match *self {
            RpType::Double => write!(f, "double"),
            RpType::Float => write!(f, "float"),
            RpType::Signed { ref size } => {
                if let Some(size) = *size {
                    write!(f, "signed/{}", size)
                } else {
                    write!(f, "signed")
                }
            }
            RpType::Unsigned { ref size } => {
                if let Some(size) = *size {
                    write!(f, "unsigned/{}", size)
                } else {
                    write!(f, "unsigned")
                }
            }
            RpType::Boolean => write!(f, "boolean"),
            RpType::String => write!(f, "string"),
            RpType::Name { ref name } => {
                if let Some(ref used) = name.prefix {
                    write!(f, "{}::{}", used, name.parts.join("."))
                } else {
                    write!(f, "{}", name.parts.join("."))
                }
            }
            RpType::Array { ref inner } => write!(f, "[{}]", inner),
            RpType::Map { ref key, ref value } => write!(f, "{{{}: {}}}", key, value),
            RpType::Any => write!(f, "any"),
            RpType::Bytes => write!(f, "bytes"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RpUseDecl {
    pub package: Loc<RpPackage>,
    pub version_req: Option<Loc<VersionReq>>,
    pub alias: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum RpValue {
    String(String),
    Number(RpNumber),
    Boolean(bool),
    Identifier(String),
    Array(Vec<Loc<RpValue>>),
    Object(Loc<RpObject>),
}

impl RpValue {
    pub fn as_match_kind(&self) -> RpMatchKind {
        match *self {
            RpValue::String(_) => RpMatchKind::String,
            RpValue::Number(_) => RpMatchKind::Number,
            RpValue::Boolean(_) => RpMatchKind::Boolean,
            RpValue::Identifier(_) => RpMatchKind::Any,
            RpValue::Array(_) => RpMatchKind::Array,
            RpValue::Object(_) => RpMatchKind::Any,
        }
    }

    pub fn as_str(&self) -> Result<&str> {
        match *self {
            RpValue::String(ref string) => Ok(string),
            _ => Err("not a string".into()),
        }
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
            RpValue::Object(_) => "<object>",
        };

        write!(f, "{}", out)
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct RpVersionedPackage {
    pub package: Option<RpPackage>,
    pub version: Option<Version>,
}

impl RpVersionedPackage {
    pub fn new(package: Option<RpPackage>, version: Option<Version>) -> RpVersionedPackage {
        RpVersionedPackage {
            package: package,
            version: version,
        }
    }

    pub fn into_type_id(&self, name: RpName) -> RpTypeId {
        RpTypeId::new(self.clone(), name)
    }

    pub fn into_package<F>(&self, version_fn: F) -> RpPackage
    where
        F: FnOnce(&Version) -> String,
    {
        let mut parts = Vec::new();

        if let Some(ref package) = self.package {
            parts.extend(package.parts.iter().map(Clone::clone));
        }

        if let Some(ref version) = self.version {
            parts.push(version_fn(version));
        }

        RpPackage::new(parts)
    }
}

impl fmt::Display for RpVersionedPackage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(ref package) = self.package {
            write!(f, "{}", package)?;
        } else {
            write!(f, "*empty*")?;
        }

        if let Some(ref version) = self.version {
            write!(f, "@{}", version)?;
        }

        Ok(())
    }
}
