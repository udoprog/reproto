use super::rp_name::RpName;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum RpType {
    Double,
    Float,
    Signed(Option<usize>),
    Unsigned(Option<usize>),
    Boolean,
    String,
    Bytes,
    Any,
    Name(RpName),
    Array(Box<RpType>),
    Map(Box<RpType>, Box<RpType>),
}

impl ::std::fmt::Display for RpType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        match *self {
            RpType::Double => write!(f, "double"),
            RpType::Float => write!(f, "float"),
            RpType::Signed(ref size) => {
                if let Some(size) = *size {
                    write!(f, "signed/{}", size)
                } else {
                    write!(f, "signed")
                }
            }
            RpType::Unsigned(ref size) => {
                if let Some(size) = *size {
                    write!(f, "unsigned/{}", size)
                } else {
                    write!(f, "unsigned")
                }
            }
            RpType::Boolean => write!(f, "boolean"),
            RpType::String => write!(f, "string"),
            RpType::Name(ref name) => {
                if let Some(ref used) = name.prefix {
                    write!(f, "{}::{}", used, name.parts.join("."))
                } else {
                    write!(f, "{}", name.parts.join("."))
                }
            }
            RpType::Array(ref inner) => write!(f, "[{}]", inner),
            RpType::Map(ref key, ref value) => write!(f, "{{{}: {}}}", key, value),
            RpType::Any => write!(f, "any"),
            RpType::Bytes => write!(f, "bytes"),
        }
    }
}
