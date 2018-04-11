//! The context of a single execution.
//!
//! Is used to accumulate errors.
//!
//! This is preferred over results, since it permits reporting complex errors and their
//! corresponding locations.

use errors::{Error, Result};
use std::cell::{BorrowError, Ref, RefCell};
use std::fmt;
use std::path::Path;
use std::rc::Rc;
use std::result;
use {ErrorPos, Filesystem, Handle};

pub enum ContextItem {
    /// A positional error.
    ErrorPos(ErrorPos, String),
    /// A positional information string.
    InfoPos(ErrorPos, String),
}

#[derive(Clone)]
/// Context for a single reproto run.
pub struct Context {
    /// Filesystem abstraction.
    filesystem: Rc<Box<Filesystem>>,
    /// Collected context errors.
    errors: Rc<RefCell<Vec<ContextItem>>>,
}

/// A reporter that processes the given error for the context.
///
/// Converting the reporter into an `ErrorKind` causes it to accumulate the errors to the `Context`.
pub struct Reporter<'a> {
    ctx: &'a Context,
    errors: Vec<ContextItem>,
}

impl<'a> Reporter<'a> {
    pub fn err<P: Into<ErrorPos>, E: fmt::Display>(mut self, pos: P, error: E) -> Self {
        self.errors
            .push(ContextItem::ErrorPos(pos.into(), error.to_string()));

        self
    }

    pub fn info<P: Into<ErrorPos>, I: fmt::Display>(mut self, pos: P, info: I) -> Self {
        self.errors
            .push(ContextItem::InfoPos(pos.into(), info.to_string()));

        self
    }

    /// Close the reporter, saving any reported errors to the context.
    pub fn close(self) -> Option<Error> {
        if self.errors.is_empty() {
            return None;
        }

        let ctx = self.ctx;

        let mut errors = ctx.errors
            .try_borrow_mut()
            .expect("exclusive mutable access");

        errors.extend(self.errors);
        Some(Error::new("Error in Context"))
    }

    /// Check if reporter is empty.
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }
}

impl<'a> From<Reporter<'a>> for Error {
    fn from(reporter: Reporter<'a>) -> Error {
        reporter.close();
        Error::new("Error in Context")
    }
}

impl Context {
    /// Create a new context with the given filesystem.
    pub fn new(filesystem: Box<Filesystem>) -> Context {
        Context {
            filesystem: Rc::new(filesystem),
            errors: Rc::new(RefCell::new(vec![])),
        }
    }

    /// Modify the existing context with a reference to the given errors.
    pub fn with_errors(self, errors: Rc<RefCell<Vec<ContextItem>>>) -> Context {
        Context {
            errors: errors,
            ..self
        }
    }

    /// Map the existing filesystem and return a new context with the new filesystem.
    pub fn map_filesystem<F>(self, map: F) -> Self
    where
        F: FnOnce(Rc<Box<Filesystem>>) -> Box<Filesystem>,
    {
        Context {
            filesystem: Rc::new(map(self.filesystem)),
            ..self
        }
    }

    /// Retrieve the filesystem abstraction.
    pub fn filesystem(&self, root: Option<&Path>) -> Result<Box<Handle>> {
        self.filesystem.open_root(root)
    }

    /// Build a handle that can be used in conjunction with Result#map_err.
    pub fn report(&self) -> Reporter {
        Reporter {
            ctx: self,
            errors: Vec::new(),
        }
    }

    /// Iterate over all reporter errors.
    pub fn errors(&self) -> result::Result<Ref<Vec<ContextItem>>, BorrowError> {
        self.errors.try_borrow()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fs::CapturingFilesystem;
    use object::{BytesObject, Object};
    use pos::Pos;
    use std::rc::Rc;
    use std::result;
    use std::sync::Arc;

    #[test]
    fn test_handle() {
        let object = BytesObject::new("test".to_string(), Arc::new(Vec::new()));

        let pos: Pos = (Rc::new(object.clone_object()), 0usize, 0usize).into();
        let other_pos: Pos = (Rc::new(object.clone_object()), 0usize, 0usize).into();

        let ctx = Context::new(Box::new(CapturingFilesystem::new()));

        let result: result::Result<(), &str> = Err("nope");

        let a: Result<()> = result.map_err(|e| {
            ctx.report()
                .err(pos, e)
                .err(other_pos, "previously reported here")
                .into()
        });

        let e = a.unwrap_err();

        match e {
            ref e if e.message() == "Error in Context" => {}
            ref other => panic!("unexpected: {:?}", other),
        }

        assert_eq!(2, ctx.errors().unwrap().len());
    }
}
