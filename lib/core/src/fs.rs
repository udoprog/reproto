//! Filesystem abstractions.

use errors::Result;
use linked_hash_map::LinkedHashMap;
use std::cell::RefCell;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use {RelativePath, RelativePathBuf};

pub trait Handle {
    /// Check if the given path is a directory or not.
    fn is_dir(&self, path: &RelativePath) -> bool;

    /// Check if the given path is a file or not.
    fn is_file(&self, path: &RelativePath) -> bool;

    /// Recursively create the given path.
    fn create_dir_all(&self, path: &RelativePath) -> Result<()>;

    /// Create the given file (for writing).
    fn create(&self, path: &RelativePath) -> Result<Box<io::Write>>;
}

/// Filesystem abstraction.
pub trait Filesystem {
    /// Open the filesystem from the given root path.
    fn open_root(&self, root: Option<&Path>) -> Result<Box<Handle>>;
}

/// Real filesystem implementation.
pub struct RealFilesystem {}

impl RealFilesystem {
    pub fn new() -> RealFilesystem {
        Self {}
    }
}

impl Filesystem for RealFilesystem {
    fn open_root(&self, root: Option<&Path>) -> Result<Box<Handle>> {
        let root = root.ok_or_else(|| {
            "Missing root directory, specify using `--out`, or `output` key in manifest"
        })?
            .to_owned();

        return Ok(Box::new(RealHandle { root: root }));

        struct RealHandle {
            root: PathBuf,
        }

        impl Handle for RealHandle {
            fn is_dir(&self, path: &RelativePath) -> bool {
                path.to_path(&self.root).is_dir()
            }

            fn is_file(&self, path: &RelativePath) -> bool {
                path.to_path(&self.root).is_file()
            }

            fn create_dir_all(&self, path: &RelativePath) -> Result<()> {
                let path = path.to_path(&self.root);
                Ok(fs::create_dir_all(&path)?)
            }

            fn create(&self, path: &RelativePath) -> Result<Box<io::Write>> {
                let path = path.to_path(&self.root);
                Ok(Box::new(fs::File::create(&path)?))
            }
        }
    }
}

/// Capture all filesystem operations in-memory.
///
/// Used (among other things) for rendering output in WASM.
pub struct CapturingFilesystem {
    files: Rc<RefCell<LinkedHashMap<RelativePathBuf, Vec<u8>>>>,
}

impl CapturingFilesystem {
    pub fn new() -> CapturingFilesystem {
        Self {
            files: Rc::new(RefCell::new(LinkedHashMap::new())),
        }
    }

    /// Create a new filesystem handle that can be passed into `Context`.
    pub fn filesystem(&self) -> Box<Filesystem> {
        Box::new(CapturingFilesystem {
            files: self.files.clone(),
        })
    }

    /// Access the underlying captured files.
    pub fn files(&self) -> &Rc<RefCell<LinkedHashMap<RelativePathBuf, Vec<u8>>>> {
        &self.files
    }
}

impl Filesystem for CapturingFilesystem {
    fn open_root(&self, _root: Option<&Path>) -> Result<Box<Handle>> {
        return Ok(Box::new(CapturingHandle {
            files: self.files.clone(),
        }));
    }
}

/// A handle that captures files into a RefCell.
struct CapturingHandle {
    files: Rc<RefCell<LinkedHashMap<RelativePathBuf, Vec<u8>>>>,
}

impl Handle for CapturingHandle {
    fn is_dir(&self, _path: &RelativePath) -> bool {
        true
    }

    fn is_file(&self, path: &RelativePath) -> bool {
        self.files.borrow().contains_key(path)
    }

    fn create_dir_all(&self, _path: &RelativePath) -> Result<()> {
        Ok(())
    }

    fn create(&self, path: &RelativePath) -> Result<Box<io::Write>> {
        Ok(Box::new(CapturingFileCreate {
            files: self.files.clone(),
            path: path.to_owned(),
            buffer: Vec::new(),
        }))
    }
}

/// An 'open file' for the capturing handle.
struct CapturingFileCreate {
    files: Rc<RefCell<LinkedHashMap<RelativePathBuf, Vec<u8>>>>,
    path: RelativePathBuf,
    buffer: Vec<u8>,
}

impl io::Write for CapturingFileCreate {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.buffer.flush()
    }
}

impl Drop for CapturingFileCreate {
    fn drop(&mut self) {
        let mut files = self.files.borrow_mut();
        files.insert(self.path.clone(), self.buffer.clone());
    }
}
