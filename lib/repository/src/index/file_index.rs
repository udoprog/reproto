use checksum::Checksum;
use core::errors::*;
use core::{Range, RelativePath, RpPackage, Version};
use index::{Deployment, Index};
use objects::{FileObjects, Objects};
use serde_json;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};

/// Default to objects relative to index repo.
const DEFAULT_OBJECTS: &'static str = "./objects";
/// Index configuration file.
const CONFIG_JSON: &'static str = "config.json";
/// Name of metadata file for each package.
const METADATA_JSON: &'static str = "metadata.json";

fn default_objects() -> String {
    DEFAULT_OBJECTS.to_owned()
}

#[derive(Deserialize, Serialize)]
pub struct Config {
    #[serde(default = "default_objects")]
    objects: String,
}

pub struct FileIndex {
    path: PathBuf,
    config: Config,
}

impl FileIndex {
    pub fn new<P: AsRef<Path> + ?Sized>(path: &P) -> Result<FileIndex> {
        let config = read_config(path)?;

        Ok(FileIndex {
            path: path.as_ref().to_owned(),
            config: config,
        })
    }

    /// Path to root of file index
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// # read package metadata
    ///
    /// returns a tuple where the first is all matching deployments, and the second is a boolean
    /// indicating if some deployments have been omitted or not.
    fn read_package<F>(&self, package: &RpPackage, filter: F) -> Result<(Vec<Deployment>, bool)>
    where
        F: Fn(&Deployment) -> bool,
    {
        let path = self.path_for(package).join(METADATA_JSON);

        if !path.is_file() {
            return Ok((vec![], false));
        }

        let f = File::open(&path)?;
        let reader = BufReader::new(f);
        let mut out = Vec::new();
        let mut non_match = false;

        for (i, line) in reader.lines().enumerate() {
            let line = line?;

            let deployment: Deployment = serde_json::from_str(&line).map_err(|e| {
                format!(
                    "{}: bad deployment on line #{}: {}",
                    path.display(),
                    i + 1,
                    e
                )
            })?;

            if filter(&deployment) {
                out.push(deployment);
            } else {
                non_match = true;
            }
        }

        Ok((out, non_match))
    }

    fn write_package<I>(&self, package: &RpPackage, deployments: I) -> Result<()>
    where
        I: IntoIterator<Item = Deployment>,
    {
        let target = self.path_for(package).join(METADATA_JSON);
        debug!("writing: {}", target.display());

        let mut tmp_target = target.clone();
        tmp_target.set_extension(".tmp");

        if let Some(parent) = tmp_target.parent() {
            if !parent.is_dir() {
                debug!("creating directory: {}", parent.display());
                fs::create_dir_all(parent)?;
            }
        }

        {
            let mut f = File::create(&tmp_target)?;
            let it = deployments.into_iter();

            for deployment in it {
                writeln!(f, "{}", serde_json::to_string(&deployment)?)?;
            }
        }

        fs::rename(&tmp_target, &target)?;
        Ok(())
    }

    fn path_for(&self, package: &RpPackage) -> PathBuf {
        package
            .parts
            .iter()
            .fold(self.path.clone(), |path, next| path.join(next))
    }
}

impl Index for FileIndex {
    fn resolve(&self, package: &RpPackage, range: &Range) -> Result<Vec<Deployment>> {
        self.read_package(package, |d| range.matches(&d.version))
            .map(|r| r.0)
    }

    fn all(&self, package: &RpPackage) -> Result<Vec<Deployment>> {
        self.read_package(package, |_| true)
            .map(|r| r.0)
            .map(|all| {
                // get the last deployment available
                all.into_iter().collect::<Vec<_>>()
            })
    }

    fn put_version(
        &self,
        checksum: &Checksum,
        package: &RpPackage,
        version: &Version,
        force: bool,
    ) -> Result<()> {
        let (mut deployments, other_match) = self.read_package(package, |d| d.version != *version)?;

        if other_match {
            if !force {
                return Err(format!("{}@{}: already published", package, version).into());
            }
        }

        deployments.push(Deployment::new(version.clone(), checksum.clone()));
        deployments.sort_by(|a, b| a.version.cmp(&b.version));
        self.write_package(package, deployments)?;
        Ok(())
    }

    fn get_deployments(&self, package: &RpPackage, version: &Version) -> Result<Vec<Deployment>> {
        self.read_package(package, |d| d.version == *version)
            .map(|r| r.0)
    }

    fn objects_from_index(&self, relative_path: &RelativePath) -> Result<Box<Objects>> {
        let path = relative_path.to_path(&self.path);
        Ok(Box::new(FileObjects::new(&path)))
    }

    fn objects_url(&self) -> Result<&str> {
        Ok(self.config.objects.as_str())
    }
}

pub fn init_file_index<P: AsRef<Path> + ?Sized>(path: &P) -> Result<()> {
    let path = path.as_ref();

    if !path.is_dir() {
        fs::create_dir_all(path)?;
    }

    let config_path = path.join(CONFIG_JSON);

    if !config_path.is_file() {
        let mut f = File::create(config_path)?;
        let config = Config {
            objects: DEFAULT_OBJECTS.to_owned(),
        };
        let config_content = serde_json::to_value(&config)?;
        writeln!(f, "{:#}", config_content)?;
    }

    Ok(())
}

/// Read the configuration from the given path.
fn read_config<P: AsRef<Path> + ?Sized>(path: &P) -> Result<Config> {
    let path = path.as_ref();

    if !path.is_dir() {
        return Err(format!("{}: not a directory", path.display()).into());
    }

    let config_path = path.join(CONFIG_JSON);

    if !config_path.is_file() {
        return Err(format!("{}: not an index, missing {}", path.display(), CONFIG_JSON).into());
    }

    let mut f = File::open(&config_path)
        .map_err(|e| format!("failed to open {}: {}", config_path.display(), e))?;

    let mut content = String::new();
    f.read_to_string(&mut content)?;

    let config: Config = serde_json::from_str(content.as_str())
        .map_err(|e| format!("{}: bad config file: {}", config_path.display(), e))?;

    Ok(config)
}
