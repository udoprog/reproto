//! ## Load objects through a local cache directory

use Objects;
use checksum::Checksum;
use core::Source;
use core::errors::*;
use hex_slice::HexSlice;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::PathBuf;
use std::time::{self, Duration};

pub struct CachedObjects<T> {
    objects_cache: PathBuf,
    missing_cache_time: Duration,
    inner: T,
}

impl<T: Objects> CachedObjects<T> {
    pub fn new(objects_cache: PathBuf, missing_cache_time: Duration, inner: T) -> CachedObjects<T> {
        CachedObjects {
            objects_cache: objects_cache,
            missing_cache_time: missing_cache_time,
            inner: inner,
        }
    }

    fn cache_path(&self, checksum: &Checksum) -> Result<PathBuf> {
        let path = self.objects_cache
            .join(format!("{}", HexSlice::new(&checksum[0..1])));
        let path = path.join(format!("{}", HexSlice::new(&checksum[1..2])));
        Ok(path.join(format!("{}", HexSlice::new(&checksum))))
    }

    /// Get the path to the missing file cache.
    fn missing_path(&self, checksum: &Checksum) -> Result<PathBuf> {
        Ok(self.objects_cache
            .join("missing")
            .join(format!("{}", HexSlice::new(checksum))))
    }

    /// Check if there is a local missing cached file, and assume that the remote file is missing
    /// if it is present, or younger than `missing_cache_time`.
    ///
    /// Returns Some(cache_path) if the file might exist.
    fn check_missing(&self, checksum: &Checksum) -> Result<(bool, PathBuf)> {
        let path = self.missing_path(checksum)?;

        match fs::metadata(&path) {
            Err(e) => {
                if e.kind() != io::ErrorKind::NotFound {
                    return Err(e.into());
                }
            }
            Ok(m) => {
                let now = time::SystemTime::now();
                let age = now.duration_since(m.modified()?)?;

                let expires = self.missing_cache_time
                    .checked_sub(age)
                    .unwrap_or_else(|| Duration::new(0, 0));

                debug!(
                    "cache: missing file exists: {} (age: {}s, expires: {}s)",
                    path.display(),
                    age.as_secs(),
                    expires.as_secs()
                );

                // remote file is expected to be missing
                if age < self.missing_cache_time {
                    return Ok((true, path));
                }

                debug!("cache: removing missing entry: {}", path.display());
                fs::remove_file(&path)?;
            }
        }

        Ok((false, path))
    }
}

impl<T: Objects> Objects for CachedObjects<T> {
    fn put_object(&mut self, checksum: &Checksum, source: &mut Read, force: bool) -> Result<bool> {
        self.inner.put_object(checksum, source, force)
    }

    fn get_object(&mut self, checksum: &Checksum) -> Result<Option<Source>> {
        let cache_path = self.cache_path(checksum)?;

        if cache_path.is_file() {
            return Ok(Some(Source::from_path(cache_path)));
        }

        let (missing, missing_path) = self.check_missing(checksum)?;

        if missing {
            return Ok(None);
        }

        let out = self.inner.get_object(checksum)?;

        if let Some(object) = out {
            if let Some(parent) = cache_path.parent() {
                if !parent.is_dir() {
                    fs::create_dir_all(parent)?;
                }
            }

            io::copy(&mut object.read()?, &mut File::create(cache_path)?)?;
            return Ok(Some(object));
        } else {
            // write cache entry indicating that there is nothing in the remote entry to avoid
            // subsequent requests.
            debug!(
                "cache: creating missing cache entry: {}",
                missing_path.display()
            );

            if let Some(parent) = missing_path.parent() {
                if !parent.is_dir() {
                    fs::create_dir_all(parent)?;
                }
            }

            File::create(missing_path)?;
        }

        return Ok(None);
    }
}
