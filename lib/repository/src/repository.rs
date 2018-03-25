use super::Objects;
use core::errors::*;
use core::{self, Object, Resolved, ResolvedByPrefix, Resolver, RpPackage, RpRequiredPackage,
           Version};
use index::{Deployment, Index};
use sha256::to_sha256;
use update::Update;

pub struct Repository {
    index: Box<Index>,
    objects: Box<Objects>,
}

impl Repository {
    pub fn new(index: Box<Index>, objects: Box<Objects>) -> Repository {
        Repository {
            index: index,
            objects: objects,
        }
    }

    pub fn update(&self) -> Result<Vec<Update>> {
        let mut updates = Vec::new();
        updates.extend(self.index.update()?);
        updates.extend(self.objects.update()?);
        Ok(updates)
    }

    pub fn publish<O>(
        &mut self,
        object: O,
        package: &RpPackage,
        version: &Version,
        force: bool,
    ) -> Result<()>
    where
        O: AsRef<Object>,
    {
        if !self.index.get_deployments(package, version)?.is_empty() {
            if !force {
                return Err(format!("{}@{}: already published", package, version).into());
            } else {
                info!("{}@{}: already published (forced)", package, version);
            }
        }

        let object = object.as_ref();
        let checksum = to_sha256(object.read()?)?;

        self.objects
            .put_object(&checksum, &mut object.read()?, force)?;
        self.index.put_version(&checksum, package, version, force)?;

        Ok(())
    }

    /// Get all deployments in this repository.
    pub fn all(&self, package: &RpPackage) -> Result<Vec<Deployment>> {
        self.index.all(package)
    }

    /// Get the object for the specific deployment.
    pub fn get_object(&mut self, deployment: &Deployment) -> Result<Option<Box<Object>>> {
        self.objects.get_object(&deployment.object)
    }
}

impl Resolver for Repository {
    fn resolve(&mut self, package: &RpRequiredPackage) -> core::errors::Result<Vec<Resolved>> {
        let mut out = Vec::new();

        let deployments = self.index.resolve(&package.package, &package.range)?;

        for deployment in deployments {
            if let Some(path) = self.get_object(&deployment)? {
                out.push(Resolved {
                    version: Some(deployment.version),
                    object: path,
                });
            } else {
                return Err(format!("missing object: {}", deployment.object).into());
            }
        }

        Ok(out)
    }

    fn resolve_by_prefix(&mut self, _: &RpPackage) -> core::errors::Result<Vec<ResolvedByPrefix>> {
        Err("repository does not support resolve by prefix".into())
    }
}
