use parser::ast;
use super::errors::*;
use super::into_model::IntoModel;
use super::rp_instance::RpInstance;
use super::rp_loc::{RpLoc, RpPos};
use super::rp_match_kind::RpMatchKind;
use super::rp_name::RpName;
use super::rp_number::RpNumber;
use super::rp_path::RpPath;
use super::rp_type::RpType;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum RpValue {
    Path(RpPath),
    String(String),
    Number(RpNumber),
    Boolean(bool),
    Identifier(String),
    Type(RpType),
    Instance(RpLoc<RpInstance>),
    Constant(RpLoc<RpName>),
    Array(Vec<RpLoc<RpValue>>),
}

impl RpValue {
    pub fn as_match_kind(&self) -> RpMatchKind {
        match *self {
            RpValue::Path(_) => RpMatchKind::String,
            RpValue::String(_) => RpMatchKind::String,
            RpValue::Number(_) => RpMatchKind::Number,
            RpValue::Boolean(_) => RpMatchKind::Boolean,
            RpValue::Identifier(_) => RpMatchKind::Any,
            RpValue::Type(_) => RpMatchKind::Any,
            RpValue::Instance(_) => RpMatchKind::Object,
            RpValue::Constant(_) => RpMatchKind::Any,
            RpValue::Array(_) => RpMatchKind::Array,
        }
    }
}

impl ::std::fmt::Display for RpValue {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        let out = match *self {
            RpValue::Path(_) => "<path>",
            RpValue::String(_) => "<string>",
            RpValue::Number(_) => "<number>",
            RpValue::Boolean(_) => "<boolean>",
            RpValue::Identifier(_) => "<identifier>",
            RpValue::Type(_) => "<type>",
            RpValue::Instance(_) => "<instance>",
            RpValue::Constant(_) => "<constant>",
            RpValue::Array(_) => "<array>",
        };

        write!(f, "{}", out)
    }
}

impl IntoModel for ast::Value {
    type Output = RpValue;

    fn into_model(self, pos: &RpPos) -> Result<RpValue> {
        let out = match self {
            ast::Value::String(string) => RpValue::String(string),
            ast::Value::Number(number) => RpValue::Number(number),
            ast::Value::Boolean(boolean) => RpValue::Boolean(boolean),
            ast::Value::Identifier(identifier) => RpValue::Identifier(identifier),
            ast::Value::Type(ty) => RpValue::Type(ty),
            ast::Value::Instance(instance) => RpValue::Instance(instance.into_model(pos)?),
            ast::Value::Constant(name) => RpValue::Constant(name.into_model(pos)?),
            ast::Value::Array(inner) => RpValue::Array(inner.into_model(pos)?),
            _ => return Err(format!("not supported: {:?}", self).into()),
        };

        Ok(out)
    }
}
