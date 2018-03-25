use errors::Result;
use std::fmt;
use {Object, RpPackage, RpRequiredPackage, Version};

/// A resolved package.
#[derive(Debug)]
pub struct Resolved {
    /// Version of object found.
    pub version: Option<Version>,
    /// Object found.
    pub object: Box<Object>,
}

/// A resolved package.
#[derive(Debug)]
pub struct ResolvedByPrefix {
    /// Package object belongs to.
    pub package: RpPackage,
    /// Object found.
    pub object: Box<Object>,
}

impl Resolved {
    /// Build a resolved object from a tuple pair.
    pub fn from_pair(pair: (Option<Version>, Box<Object>)) -> Resolved {
        let (version, object) = pair;

        Resolved {
            version: version,
            object: object,
        }
    }
}

impl fmt::Display for Resolved {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        if let Some(ref version) = self.version {
            write!(fmt, "{}-{}", self.object, version)
        } else {
            self.object.fmt(fmt)
        }
    }
}

/// Trait that translates a required package into a set of versions and objects.
pub trait Resolver {
    /// Resolve the specified request.
    fn resolve(&mut self, package: &RpRequiredPackage) -> Result<Vec<Resolved>>;

    /// Resolve by prefix.
    fn resolve_by_prefix(&mut self, package: &RpPackage) -> Result<Vec<ResolvedByPrefix>>;
}

pub struct EmptyResolver;

impl Resolver for EmptyResolver {
    fn resolve(&mut self, _package: &RpRequiredPackage) -> Result<Vec<Resolved>> {
        Ok(vec![])
    }

    fn resolve_by_prefix(&mut self, _package: &RpPackage) -> Result<Vec<ResolvedByPrefix>> {
        Ok(vec![])
    }
}
