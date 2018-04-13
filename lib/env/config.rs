//! Utilities for loading configuration files.

use core::errors::*;
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
    pub cache_home: Option<PathBuf>,
}

pub fn read_config<P: AsRef<Path>>(path: P) -> Result<Config> {
    let path = path.as_ref();
    let mut f = File::open(path)?;
    let mut content = String::new();
    f.read_to_string(&mut content)?;
    let config: Config = toml::from_str(content.as_str())
        .map_err(|e| format!("{}: bad config: {}", path.display(), e))?;
    Ok(config)
}
