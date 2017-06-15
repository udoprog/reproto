use super::rp_name::RpName;
use super::rp_type_id::RpTypeId;

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct RpPackage {
    pub parts: Vec<String>,
}

impl RpPackage {
    pub fn new(parts: Vec<String>) -> RpPackage {
        RpPackage { parts: parts }
    }

    pub fn join(&self, other: &RpPackage) -> RpPackage {
        let mut parts = self.parts.clone();
        parts.extend(other.parts.clone());
        RpPackage::new(parts)
    }

    pub fn into_type_id(&self, name: &RpName) -> RpTypeId {
        RpTypeId::new(self.clone(), name.clone())
    }
}

impl ::std::fmt::Display for RpPackage {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "{}", self.parts.join("."))
    }
}
