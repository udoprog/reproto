//! Encapsulate home environment

use config::read_config;
use core::errors::Result;
use std::env;
use std::path::PathBuf;

mod unix {
    use core::errors::Result;
    use std::env;
    use std::path::{Path, PathBuf};

    /// Access the XDG directory.
    fn xdg_dir<D>(home: Option<&PathBuf>, var: &str, default: D) -> Result<Option<PathBuf>>
    where
        D: FnOnce(&PathBuf) -> PathBuf,
    {
        match env::var(var) {
            Ok(dir) => return Ok(Some(Path::new(&dir).join("reproto"))),
            Err(env::VarError::NotPresent) => Ok(home.map(|home| default(home).join("reproto"))),
            Err(e) => return Err(format!("env {} failed: {}", var, e).into()),
        }
    }

    /// Get the configuration directory.
    pub fn config_dir(home: Option<&PathBuf>) -> Result<Option<PathBuf>> {
        xdg_dir(home, "XDG_CONFIG_DIR", |p| p.join(".config"))
    }

    /// Get the cache directory.
    pub fn cache_home(home: Option<&PathBuf>) -> Result<Option<PathBuf>> {
        xdg_dir(home, "XDG_CACHE_HOME", |p| p.join(".cache"))
    }

    /// Data home directory.
    pub fn data_home(home: Option<&PathBuf>) -> Result<Option<PathBuf>> {
        xdg_dir(home, "XDG_DATA_HOME", |p| p.join(".local").join("share"))
    }

    /// Bin home
    pub fn bin_home(home: Option<&PathBuf>) -> Result<Option<PathBuf>> {
        Ok(home.map(|home| home.join(".local").join("bin")))
    }
}

use self::unix::*;

pub struct ConfigEnv {
    pub config: PathBuf,
    pub repo_dir: PathBuf,
    pub cache_home: PathBuf,
    pub releases_dir: PathBuf,
    pub bin_home: PathBuf,
    pub index: Option<String>,
    pub objects: Option<String>,
}

impl ConfigEnv {
    pub fn new() -> Result<Option<ConfigEnv>> {
        let home = env::home_dir();

        let config_dir = match config_dir(home.as_ref())? {
            None => return Ok(None),
            Some(config_dir) => config_dir,
        };

        let mut cache_home = match cache_home(home.as_ref())? {
            None => return Ok(None),
            Some(cache_home) => cache_home,
        };

        let data_home = match data_home(home.as_ref())? {
            None => return Ok(None),
            Some(data_home) => data_home,
        };

        let bin_home = match bin_home(home.as_ref())? {
            None => return Ok(None),
            Some(bin_home) => bin_home,
        };

        let config = config_dir.join("config.toml");
        let mut repo_dir = config_dir.join("git");

        let mut index = None;
        let mut objects = None;

        if config.is_file() {
            let config = read_config(&config)?;

            if let Some(repository) = config.repository {
                // set values from configuration (if not already set).
                index = index.or(repository.index);
                objects = objects.or(repository.objects);
            }

            if let Some(out) = config.cache_home {
                cache_home = out;
            }

            if let Some(out) = config.repo_dir {
                repo_dir = out;
            }
        }

        let releases_dir = data_home.join("releases");

        return Ok(Some(ConfigEnv {
            config,
            repo_dir,
            cache_home,
            releases_dir,
            bin_home,
            index,
            objects,
        }));
    }
}
