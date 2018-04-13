//! ## Load objects through a local git repo

use checksum::Checksum;
use core::Source;
use core::errors::*;
use git::GitRepo;
use objects::{FileObjects, Objects};
use std::io::Read;
use std::rc::Rc;
use update::Update;
use url::Url;

pub struct GitObjects {
    url: Url,
    git_repo: Rc<GitRepo>,
    file_objects: FileObjects,
    publishing: bool,
}

impl GitObjects {
    pub fn new(
        url: Url,
        git_repo: Rc<GitRepo>,
        file_objects: FileObjects,
        publishing: bool,
    ) -> GitObjects {
        GitObjects {
            url,
            git_repo,
            file_objects,
            publishing,
        }
    }
}

impl Objects for GitObjects {
    fn put_object(&mut self, checksum: &Checksum, reader: &mut Read, force: bool) -> Result<bool> {
        if !self.publishing {
            return Err(format!("objects repo not support publishing: {}", self.url).into());
        }

        let added = self.file_objects.put_object(checksum, reader, force)?;

        if added {
            // add the newly added file.
            let path = self.file_objects.checksum_path(checksum)?;
            self.git_repo.add(path)?;
        }

        Ok(added)
    }

    fn get_object(&mut self, checksum: &Checksum) -> Result<Option<Source>> {
        self.file_objects.get_object(checksum)
    }

    fn update(&self) -> Result<Vec<Update>> {
        Ok(vec![Update::GitRepo(&self.git_repo)])
    }
}
