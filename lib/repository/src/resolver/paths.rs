//! # Default paths-based resolver
//!
//! Resolves packages based on a set of paths.
//!
//! These paths have the following structure:
//!
//! * `<root>/<package>/<last>.reproto`
//! * `<root>/<package>/<last>/<version>.reproto`
//!
//! The second form is only used when a version requirement is present.

use core::errors::{Result, ResultExt};
use core::{Object, PathObject, Range, Resolved, ResolvedByPrefix, Resolver, RpPackage,
           RpRequiredPackage, Version};
use std::collections::{BTreeMap, HashMap, LinkedList};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

const EXT: &str = "reproto";

pub struct Paths {
    /// Paths to perform lookups in.
    paths: Vec<PathBuf>,
    /// Entries which are locally published.
    published: HashMap<RpPackage, Version>,
}

impl Paths {
    pub fn new(paths: Vec<PathBuf>, published: HashMap<RpPackage, Version>) -> Paths {
        Paths {
            paths: paths,
            published: published,
        }
    }

    fn parse_stem<'a>(&self, stem: &'a str) -> Result<(&'a str, Option<Version>)> {
        let mut it = stem.splitn(2, '-');

        if let (Some(name_base), Some(name_version)) = (it.next(), it.next()) {
            let version = Version::parse(name_version).map_err(|_| format!("bad version"))?;

            return Ok((name_base, Some(version)));
        }

        Ok((stem, None))
    }

    /// Finds the published version from most to least specific package.
    pub fn find_published_version(&self, package: &RpPackage) -> Option<&Version> {
        if let Some(version) = self.published.get(package) {
            return Some(version);
        }

        let mut it = package.parts.iter();

        while let Some(_) = it.next_back() {
            let package = RpPackage::new(it.as_slice().to_vec());

            if let Some(version) = self.published.get(&package) {
                return Some(version);
            }
        }

        None
    }

    /// Find any matching versions.
    pub fn find_by_range(
        &self,
        path: &Path,
        base: &str,
        package: &RpPackage,
        range: &Range,
    ) -> Result<Vec<Resolved>> {
        let mut files: BTreeMap<Option<Version>, Box<Object>> = BTreeMap::new();

        for e in fs::read_dir(path)? {
            let p = e?.path();

            // only look for files
            if !p.is_file() {
                continue;
            }

            if p.extension().map(|ext| ext != EXT).unwrap_or(true) {
                continue;
            }

            if let Some(stem) = p.file_stem().and_then(OsStr::to_str) {
                let (name_base, version) = self.parse_stem(stem)
                    .chain_err(|| format!("Failed to parse stem from: {}", p.display()))?;

                if name_base != base {
                    continue;
                }

                let version = version.or_else(|| self.find_published_version(package).cloned());

                // versioned matches by requirement.
                if let Some(version) = version {
                    if range.matches(&version) {
                        files.insert(Some(version), Box::new(PathObject::new(None, &p)));
                    }

                    continue;
                }

                // unversioned only matches by wildcard.
                if range.matches_any() {
                    files.insert(None, Box::new(PathObject::new(None, &p)));
                }
            }
        }

        Ok(files.into_iter().map(Resolved::from_pair).collect())
    }

    /// Load .reproto file from the given package path if present.
    fn load_file(&self, path: &Path, prefix: &RpPackage) -> Option<ResolvedByPrefix> {
        let mut file = path.to_owned();
        file.set_extension(EXT);

        if file.is_file() {
            Some(ResolvedByPrefix {
                package: prefix.clone(),
                object: Box::new(PathObject::new(None, &file)),
            })
        } else {
            None
        }
    }

    /// Load .reproto file from path if valid reproto file and present.
    fn load_from_path(&self, path: &Path, prefix: RpPackage) -> Result<Option<ResolvedByPrefix>> {
        if path.extension().map(|ext| ext != EXT).unwrap_or(true) {
            debug!("skipping wrong file extension: {}", path.display());
            return Ok(None);
        }

        let stem = path.file_stem()
            .and_then(OsStr::to_str)
            .ok_or_else(|| format!("illegal path: {}", path.display()))?;

        let (stem, version) = self.parse_stem(stem)?;

        if version.is_some() {
            debug!("skipping versioned: {}", path.display());
            return Ok(None);
        }

        let package = prefix.join_part(stem);

        Ok(Some(ResolvedByPrefix {
            package: package,
            object: Box::new(PathObject::new(None, &path)),
        }))
    }

    /// Find all packages by prefix.
    pub fn find_by_prefix(&self, path: &Path, prefix: &RpPackage) -> Result<Vec<ResolvedByPrefix>> {
        let mut files = Vec::new();

        files.extend(self.load_file(path, prefix));

        if !path.is_dir() {
            return Ok(files);
        }

        let mut queue = LinkedList::new();
        queue.push_back((prefix.clone(), path.to_owned()));

        while let Some((prefix, path)) = queue.pop_front() {
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_file() {
                    files.extend(self.load_from_path(&path, prefix.clone())?);
                    continue;
                }

                if path.is_dir() {
                    let file_name = entry.file_name();

                    let name = file_name
                        .to_str()
                        .ok_or_else(|| format!("illegal path: {}", path.display()))?;

                    queue.push_back((prefix.clone().join_part(name), path));
                    continue;
                }
            }
        }

        Ok(files)
    }
}

impl Resolver for Paths {
    fn resolve(&mut self, package: &RpRequiredPackage) -> Result<Vec<Resolved>> {
        let mut files = Vec::new();

        for path in &self.paths {
            let mut path: PathBuf = path.to_owned();
            let mut it = package.package.parts.iter().peekable();

            while let Some(step) = it.next() {
                if it.peek().is_none() {
                    if path.is_dir() {
                        files.extend(self.find_by_range(
                            &path,
                            step,
                            &package.package,
                            &package.range,
                        )?);
                    }

                    break;
                }

                path = path.join(step);
            }
        }

        Ok(files)
    }

    fn resolve_by_prefix(&mut self, prefix: &RpPackage) -> Result<Vec<ResolvedByPrefix>> {
        let mut files = Vec::new();

        for path in &self.paths {
            let path = prefix
                .parts
                .iter()
                .fold(path.to_owned(), |p, part| p.join(part));
            files.extend(self.find_by_prefix(&path, prefix)?);
        }

        Ok(files)
    }
}
