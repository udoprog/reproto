//! ## Load objects through a local directory

use super::Objects;
use checksum::Checksum;
use core::Source;
use core::errors::*;
use hex_slice::HexSlice;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

pub struct FileObjects {
    path: PathBuf,
}

impl FileObjects {
    pub fn new<P: AsRef<Path> + ?Sized>(path: &P) -> FileObjects {
        FileObjects {
            path: path.as_ref().to_owned(),
        }
    }

    /// Calculate the path to the given checksum.
    pub fn checksum_path(&self, checksum: &Checksum) -> Result<PathBuf> {
        let path = self.path
            .join(format!("{}", HexSlice::new(&checksum[0..1])));
        let path = path.join(format!("{}", HexSlice::new(&checksum[1..2])));
        Ok(path.join(format!("{}.reproto", HexSlice::new(&checksum))))
    }
}

impl Objects for FileObjects {
    fn put_object(&mut self, checksum: &Checksum, source: &mut Read, force: bool) -> Result<bool> {
        let target = self.checksum_path(checksum)?;

        // no need to write same file again
        if target.is_file() && !force {
            return Ok(false);
        }

        if let Some(parent) = target.parent() {
            if !parent.is_dir() {
                debug!("creating directory: {}", parent.display());
                fs::create_dir_all(parent)?;
            }
        }

        let mut tmp_target = target.clone();
        tmp_target.set_extension(".tmp");

        debug!("writing: {}", target.display());
        io::copy(source, &mut File::create(&tmp_target)?)?;
        fs::rename(tmp_target, target)?;
        return Ok(true);
    }

    fn get_object(&mut self, checksum: &Checksum) -> Result<Option<Source>> {
        let target = self.checksum_path(checksum)?;

        if target.is_file() {
            return Ok(Some(Source::from_path(target)));
        }

        Ok(None)
    }
}
