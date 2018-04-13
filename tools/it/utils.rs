use Result;
use diff;
use relative_path::RelativePathBuf;
use std::collections::{HashSet, VecDeque};
use std::fmt;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Extract relative parts from the given path.
fn relative_parts<'a>(path: &'a Path, depth: usize) -> Result<Vec<&'a str>> {
    let mut parts = path.components().rev().take(depth).collect::<Vec<_>>();
    parts.reverse();

    let mut out = Vec::new();

    for c in parts {
        let c = c.as_os_str()
            .to_str()
            .ok_or_else(|| format_err!("not a string"))?;

        out.push(c);
    }

    Ok(out)
}

/// Resolve relative path from the given path with a known depth.
fn convert_relative(root: &Path, path: &Path, depth: usize) -> Result<PathBuf> {
    let parts = relative_parts(path, depth)?;
    let path = parts
        .into_iter()
        .fold(root.to_path_buf(), |p, part| p.join(part));
    Ok(path)
}

#[derive(Debug)]
pub enum Location {
    /// The location of the path is in the source.
    Source,
    /// The location of the path is in the destination.
    Dest,
}

pub struct Display<'a>(&'a Location);

impl<'a> fmt::Display for Display<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        use self::Location::*;

        match *self.0 {
            Source => write!(fmt, "source"),
            Dest => write!(fmt, "destination"),
        }
    }
}

impl Location {
    pub fn display(&self) -> Display {
        Display(self)
    }
}

#[derive(Debug)]
pub enum Diff {
    MissingDir(Location, RelativePathBuf),
    MissingFile(Location, RelativePathBuf),
    ExpectedDir(Location, RelativePathBuf),
    ExpectedFile(Location, RelativePathBuf),
    Mismatch(RelativePathBuf, RelativePathBuf, Vec<diff::Result<String>>),
}

/// Calculate a difference between two directories.
pub fn diff_recursive(src: &Path, dst: &Path, errors: &mut Vec<Diff>) -> Result<()> {
    use self::Diff::*;
    use self::Location::*;
    use io::ErrorKind::NotFound;

    let mut all: HashSet<Vec<String>> = HashSet::new();

    if src.is_dir() {
        for entry in WalkDir::new(src) {
            let entry = entry?;
            let parts = relative_parts(entry.path(), entry.depth())?;
            all.insert(parts.into_iter().map(|s| s.to_string()).collect());
        }
    }

    if dst.is_dir() {
        for entry in WalkDir::new(dst) {
            let entry = entry?;
            let parts = relative_parts(entry.path(), entry.depth())?;
            all.insert(parts.into_iter().map(|s| s.to_string()).collect());
        }
    }

    if src.is_dir() && !dst.is_dir() {
        errors.push(MissingDir(Dest, RelativePathBuf::new()));
    }

    if !src.is_dir() && dst.is_dir() {
        errors.push(MissingDir(Source, RelativePathBuf::new()));
    }

    for parts in all {
        let src = parts.iter().fold(src.to_path_buf(), |p, part| p.join(part));
        let dst = parts.iter().fold(dst.to_path_buf(), |p, part| p.join(part));

        let rel_src = parts
            .iter()
            .fold(RelativePathBuf::new(), |p, part| p.join(part));
        let rel_dst = parts
            .iter()
            .fold(RelativePathBuf::new(), |p, part| p.join(part));

        let (s, d) = (fs::metadata(&src), fs::metadata(&dst));

        match (s, d) {
            (Ok(ref s), Err(ref d)) if s.is_file() && d.kind() == NotFound => {
                errors.push(MissingFile(Dest, rel_dst));
            }
            (Err(ref s), Ok(ref d)) if d.is_file() && s.kind() == NotFound => {
                errors.push(MissingFile(Source, rel_src));
            }
            (Ok(ref s), Err(ref d)) if s.is_dir() && d.kind() == NotFound => {
                errors.push(MissingDir(Dest, rel_dst));
            }
            (Err(ref s), Ok(ref d)) if d.is_dir() && s.kind() == NotFound => {
                errors.push(MissingDir(Source, rel_src));
            }
            (Ok(ref s), Ok(ref d)) if s.is_file() && !d.is_file() => {
                errors.push(ExpectedFile(Dest, rel_dst));
            }
            (Ok(ref s), Ok(ref d)) if !s.is_file() && d.is_file() => {
                errors.push(ExpectedFile(Source, rel_src));
            }
            (Ok(ref s), Ok(ref d)) if s.is_dir() && !d.is_dir() => {
                errors.push(ExpectedDir(Dest, rel_dst));
            }
            (Ok(ref s), Ok(ref d)) if !s.is_dir() && d.is_dir() => {
                errors.push(ExpectedDir(Source, rel_src));
            }
            // both are dirs, everything is fine.
            (Ok(ref s), Ok(ref d)) if s.is_dir() && d.is_dir() => {}
            // both are file, compare contents.
            (Ok(ref s), Ok(ref d)) if s.is_file() && d.is_file() => {
                let left = read_contents(&src)?;
                let right = read_contents(&dst)?;

                let mut before = VecDeque::new();
                let mut after = 0u32;
                let mut mismatch = Vec::new();

                for diff in diff::lines(&left, &right) {
                    match diff {
                        diff::Result::Left(l) => {
                            mismatch.extend(before.drain(..));
                            mismatch.push(diff::Result::Left(l.to_string()));
                            after = 3;
                        }
                        diff::Result::Right(r) => {
                            mismatch.extend(before.drain(..));
                            mismatch.push(diff::Result::Right(r.to_string()));
                            after = 3;
                        }
                        diff::Result::Both(l, r) => {
                            let out = diff::Result::Both(l.to_string(), r.to_string());

                            if after > 0 {
                                mismatch.push(out.clone());
                                after -= 1;
                                continue;
                            }

                            if before.len() > 2 {
                                before.pop_front();
                            }

                            before.push_back(out);
                        }
                    }
                }

                if !mismatch.is_empty() {
                    errors.push(Diff::Mismatch(rel_src, rel_dst, mismatch));
                }
            }
            _ => {
                bail!(
                    "could not differentiate between {} and {}",
                    src.display(),
                    dst.display()
                );
            }
        }
    }

    return Ok(());

    fn read_contents(path: &Path) -> Result<String> {
        let mut buffer = String::new();
        File::open(path)?.read_to_string(&mut buffer)?;
        Ok(buffer)
    }
}

/// Recursively copy a directory.
pub fn copy_dir(source: &Path, target: &Path) -> Result<()> {
    use io::ErrorKind::NotFound;

    for entry in WalkDir::new(source) {
        let entry = entry?;

        let src = entry.path();
        let dst = convert_relative(target, src, entry.depth())?;

        let s = fs::metadata(&src)
            .map_err(|e| format_err!("failed to open source: {}: {}", src.display(), e))?;

        match fs::metadata(&dst) {
            Err(ref e) if s.is_dir() && e.kind() == NotFound => {
                fs::create_dir_all(&dst)?;
                continue;
            }
            Err(ref e) if s.is_file() && e.kind() == NotFound => {}
            Ok(ref d) if s.is_dir() && d.is_dir() => {
                continue;
            }
            Ok(ref d) if s.is_file() && d.is_file() => {
                if s.modified()? == d.modified()? && s.len() == d.len() {
                    continue;
                }

                fs::remove_file(&dst)?;
            }
            _ => bail!("failed to copy {} to {}", src.display(), dst.display()),
        }

        fs::copy(&src, &dst)?;
    }

    Ok(())
}
