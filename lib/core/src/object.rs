use errors::Result;
use std::fmt;
use std::fs::File;
use std::io::{self, Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub trait Object: Send + fmt::Display + fmt::Debug {
    /// Get a path to the object, if one exists.
    fn path(&self) -> Option<&Path>;

    /// Open a reader to the object.
    fn read(&self) -> Result<Box<Read>>;

    /// Lightweight cloning of this object.
    fn clone_object(&self) -> Box<Object>;

    /// Convert the current object with the given name.
    fn with_name(&self, name: String) -> Box<Object>;
}

/// An empty object.
#[derive(Debug)]
pub struct EmptyObject {
    name: Arc<String>,
}

impl EmptyObject {
    /// Create a new empty object with the given name.
    pub fn new<S: AsRef<str>>(name: S) -> EmptyObject {
        EmptyObject { name: Arc::new(name.as_ref().to_string()) }
    }
}

impl Object for EmptyObject {
    fn path(&self) -> Option<&Path> {
        None
    }

    fn read(&self) -> Result<Box<Read>> {
        Ok(Box::new(Cursor::new(&[])))
    }

    fn clone_object(&self) -> Box<Object> {
        Box::new(EmptyObject { name: Arc::clone(&self.name) })
    }

    fn with_name(&self, name: String) -> Box<Object> {
        Box::new(EmptyObject { name: Arc::new(name) })
    }
}

impl fmt::Display for EmptyObject {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "<{} (empty)>", self.name)
    }
}

/// An named object containing a fixed set of bytes.
#[derive(Debug)]
pub struct BytesObject {
    name: Arc<String>,
    bytes: Arc<Vec<u8>>,
}

impl BytesObject {
    pub fn new(name: String, bytes: Arc<Vec<u8>>) -> BytesObject {
        BytesObject {
            name: Arc::new(name),
            bytes: bytes,
        }
    }
}

/// Adapt a vector in an Arc to be used in a Cursor.
struct ArcCursor(Arc<Vec<u8>>);

impl AsRef<[u8]> for ArcCursor {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl Object for BytesObject {
    fn path(&self) -> Option<&Path> {
        None
    }

    fn read(&self) -> Result<Box<Read>> {
        Ok(Box::new(Cursor::new(ArcCursor(Arc::clone(&self.bytes)))))
    }

    fn clone_object(&self) -> Box<Object> {
        Box::new(BytesObject {
            name: Arc::clone(&self.name),
            bytes: Arc::clone(&self.bytes),
        })
    }

    fn with_name(&self, name: String) -> Box<Object> {
        Box::new(BytesObject {
            name: Arc::new(name),
            bytes: Arc::clone(&self.bytes),
        })
    }
}

impl fmt::Display for BytesObject {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "<{}>", self.name)
    }
}

#[derive(Debug)]
pub struct PathObject {
    name: Option<Arc<String>>,
    path: Arc<PathBuf>,
}

impl PathObject {
    pub fn new<P: AsRef<Path>>(name: Option<String>, path: P) -> PathObject {
        PathObject {
            name: name.map(Arc::new),
            path: Arc::new(path.as_ref().to_owned()),
        }
    }
}

impl Object for PathObject {
    fn path(&self) -> Option<&Path> {
        Some(self.path.as_ref())
    }

    fn read(&self) -> Result<Box<Read>> {
        Ok(Box::new(File::open(self.path.as_path())?))
    }

    fn clone_object(&self) -> Box<Object> {
        Box::new(PathObject {
            name: self.name.clone(),
            path: Arc::clone(&self.path),
        })
    }

    fn with_name(&self, name: String) -> Box<Object> {
        Box::new(PathObject {
            name: Some(Arc::new(name)),
            path: Arc::clone(&self.path),
        })
    }
}

impl fmt::Display for PathObject {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        if let Some(ref name) = self.name {
            write!(formatter, "{}", name)
        } else {
            write!(formatter, "{}", self.path.display())
        }
    }
}

impl From<PathObject> for Box<Object> {
    fn from(value: PathObject) -> Box<Object> {
        Box::new(value)
    }
}

pub struct StdinObject {
    name: Arc<String>,
}

impl StdinObject {
    pub fn new() -> Self {
        Self { name: Arc::new("stdin".to_string()) }
    }
}

impl Object for StdinObject {
    fn path(&self) -> Option<&Path> {
        None
    }

    fn read(&self) -> Result<Box<Read>> {
        Ok(Box::new(io::stdin()))
    }

    fn clone_object(&self) -> Box<Object> {
        Box::new(StdinObject { name: Arc::clone(&self.name) })
    }

    fn with_name(&self, name: String) -> Box<Object> {
        Box::new(StdinObject { name: Arc::new(name) })
    }
}

impl fmt::Debug for StdinObject {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("StdinObject")
            .field("name", &self.name)
            .finish()
    }
}

impl fmt::Display for StdinObject {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{}", self.name)
    }
}
