mod cached_objects;
mod file_objects;
mod git_objects;

pub use self::cached_objects::CachedObjects;
pub use self::file_objects::FileObjects;
pub use self::git_objects::GitObjects;
use checksum::Checksum;
use core::Source;
use core::errors::*;
use git;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;
use update::Update;
use url::Url;

/// Configuration file for objects backends.
pub struct ObjectsConfig {
    /// Root path when checking out local repositories.
    pub repo_dir: PathBuf,
    pub cache_home: Option<PathBuf>,
    pub missing_cache_time: Option<Duration>,
}

pub trait Objects {
    /// Put the given object into the database.
    /// This will cause the object denoted by the given checksum to be uploaded to the objects
    /// store.
    ///
    /// Returns a boolean indicating if the repo was updated or not.
    fn put_object(&mut self, checksum: &Checksum, source: &mut Read, force: bool) -> Result<bool>;

    /// Get a path to the object with the given checksum.
    /// This might cause the object to be downloaded if it's not already present in the local
    /// filesystem.
    fn get_object(&mut self, checksum: &Checksum) -> Result<Option<Source>>;

    /// Update local caches related to the object store.
    fn update(&self) -> Result<Vec<Update>> {
        Ok(vec![])
    }
}

pub struct NoObjects;

impl Objects for NoObjects {
    fn put_object(&mut self, _: &Checksum, _: &mut Read, _: bool) -> Result<bool> {
        Err("no objects".into())
    }

    fn get_object(&mut self, _: &Checksum) -> Result<Option<Source>> {
        Err("no objects".into())
    }
}

/// Load objects from a path.
pub fn objects_from_path<P: AsRef<Path>>(path: P) -> Result<FileObjects> {
    let path = path.as_ref();

    if !path.is_dir() {
        return Err(format!("no such directory: {}", path.display()).into());
    }

    Ok(FileObjects::new(path))
}

/// Load objects from a git+<scheme> URL.
///
/// The supplied `scheme` is an iterator over the current schemes (separated by `+`).
pub fn objects_from_git<'a, I>(
    config: ObjectsConfig,
    scheme: I,
    url: &'a Url,
    publishing: bool,
) -> Result<Box<Objects>>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut scheme = scheme.into_iter();

    let sub_scheme = scheme
        .next()
        .ok_or_else(|| format!("bad scheme ({}), expected git+scheme", url.scheme()))?;

    let git_repo = git::setup_git_repo(&config.repo_dir, sub_scheme, url)?;

    let file_objects = FileObjects::new(git_repo.path());

    let git_repo = Rc::new(git_repo);
    let objects = GitObjects::new(url.clone(), git_repo, file_objects, publishing);

    Ok(Box::new(objects))
}

/// Load objects from an URL.
pub fn objects_from_url<F>(
    config: ObjectsConfig,
    url: &Url,
    fallback: F,
    publishing: bool,
) -> Result<Box<Objects>>
where
    F: Fn(ObjectsConfig, &str, &Url) -> Result<Option<Box<Objects>>>,
{
    let mut scheme = url.scheme().split("+");

    let first = scheme
        .next()
        .ok_or_else(|| format!("Bad scheme in: {}", url))?;

    match first {
        "file" => objects_from_path(Path::new(url.path())).map(|o| Box::new(o) as Box<Objects>),
        "git" => objects_from_git(config, scheme, url, publishing),
        scheme => match fallback(config, scheme, url)? {
            Some(objects) => Ok(objects),
            None => return Err(format!("bad scheme: {}", scheme).into()),
        },
    }.chain_err(|| format!("load objects from url: {}", url))
}
