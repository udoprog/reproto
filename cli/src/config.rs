//! Utilities for loading configuration files.

use core::{FileManifest, Manifest, load_manifest};
use errors::*;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use toml;

#[derive(Debug, Deserialize)]
pub struct Repository {
    /// URL to index source.
    /// FIXME: Can't use Url type directly here with `url_serde`, since it's not seen as optional.
    pub index: Option<String>,
    /// URL to objects source.
    /// FIXME: Can't use Url type directly here with `url_serde`, since it's not seen as optional.
    pub objects: Option<String>,
}

impl Default for Repository {
    fn default() -> Repository {
        Repository {
            index: None,
            objects: None,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Config {
    /// Specified repository.
    pub repository: Option<Repository>,
    /// Where to store local checkouts of repos.
    pub repo_dir: Option<PathBuf>,
    /// Objects cache location.
    pub cache_dir: Option<PathBuf>,
}

pub fn read_config<P: AsRef<Path>>(path: P) -> Result<Config> {
    let path = path.as_ref();
    let mut f = File::open(path)?;
    let mut content = String::new();
    f.read_to_string(&mut content)?;
    let config: Config = toml::from_str(content.as_str()).map_err(|e| {
        format!("{}: bad config: {}", path.display(), e)
    })?;
    Ok(config)
}

pub fn read_manifest<P: AsRef<Path>>(path: P) -> Result<Manifest> {
    let path = path.as_ref();
    let mut f = File::open(path)?;
    let mut content = String::new();
    f.read_to_string(&mut content)?;
    let manifest: FileManifest = toml::from_str(content.as_str()).map_err(|e| {
        format!("{}: bad manifest: {}", path.display(), e)
    })?;

    let parent = path.parent().ok_or_else(
        || format!("missing parent directory"),
    )?;

    let manifest = load_manifest(parent, manifest)?;
    Ok(manifest)
}
