use core::errors::Result;
use core::{Resolved, ResolvedByPrefix, Resolver, RpPackage, RpRequiredPackage};

pub struct Resolvers {
    resolvers: Vec<Box<Resolver>>,
}

impl Resolvers {
    pub fn new(resolvers: Vec<Box<Resolver>>) -> Resolvers {
        Resolvers {
            resolvers: resolvers,
        }
    }
}

impl Resolver for Resolvers {
    fn resolve(&mut self, package: &RpRequiredPackage) -> Result<Vec<Resolved>> {
        let mut out = Vec::new();

        for resolver in &mut self.resolvers.iter_mut() {
            out.extend(resolver.resolve(package)?);
        }

        Ok(out)
    }

    fn resolve_by_prefix(&mut self, package: &RpPackage) -> Result<Vec<ResolvedByPrefix>> {
        let mut out = Vec::new();

        for resolver in &mut self.resolvers.iter_mut() {
            out.extend(resolver.resolve_by_prefix(package)?);
        }

        Ok(out)
    }
}
