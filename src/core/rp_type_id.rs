use super::rp_name::RpName;
use super::rp_package::RpPackage;

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct RpTypeId {
    pub package: RpPackage,
    pub name: RpName,
}

impl RpTypeId {
    pub fn new(package: RpPackage, name: RpName) -> RpTypeId {
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
