//! The context of a single execution.
//!
//! Is used to accumulate errors.
//!
//! This is preferred over results, since it permits reporting complex errors and their
//! corresponding locations.

use errors::Result;
use std::cell::{BorrowError, Ref, RefCell};
use std::path::Path;
use std::rc::Rc;
use std::result;
use {Diagnostics, Filesystem, Handle};

#[derive(Debug)]
pub enum ContextItem {
    /// An emitted diagnostics.
    Diagnostics { diagnostics: Diagnostics },
}

#[derive(Clone)]
/// Context for a single reproto run.
pub struct Context {
    /// Filesystem abstraction.
    filesystem: Rc<Box<Filesystem>>,
    /// Collected context items.
    items: Rc<RefCell<Vec<ContextItem>>>,
}

impl Context {
    /// Create a new context with the given filesystem.
    pub fn new(filesystem: Box<Filesystem>) -> Context {
        Context {
            filesystem: Rc::new(filesystem),
            items: Rc::new(RefCell::new(vec![])),
        }
    }

    /// Modify the existing context with a reference to the given errors.
    pub fn with_items(self, items: Rc<RefCell<Vec<ContextItem>>>) -> Context {
        Context { items, ..self }
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

    /// Add the given diagnostics to this context.
    pub fn diagnostics(&self, diagnostics: Diagnostics) -> Result<()> {
        self.items
            .try_borrow_mut()
            .map_err(|_| "no mutable access to context")?
            .push(ContextItem::Diagnostics { diagnostics });

        Ok(())
    }

    /// Iterate over all reporter items.
    pub fn items(&self) -> result::Result<Ref<Vec<ContextItem>>, BorrowError> {
        self.items.try_borrow()
    }

    /// Check if reporter is empty.
    pub fn has_diagnostics(&self) -> Result<bool> {
        Ok(self.items
            .try_borrow()
            .map_err(|_| "immutable access to context")?
            .iter()
            .any(|item| match *item {
                ContextItem::Diagnostics { ref diagnostics } => diagnostics.has_errors(),
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fs::CapturingFilesystem;
    use source::Source;
    use span::Span;
    use std::result;
    use std::sync::Arc;

    #[test]
    fn test_handle() {
        let source = Source::bytes("test", Vec::new());

        let span: Span = (Arc::new(source.clone()), 0usize, 0usize).into();
        let other_pos: Span = (Arc::new(source.clone()), 0usize, 0usize).into();

        let ctx = Context::new(Box::new(CapturingFilesystem::new()));

        let result: result::Result<(), &str> = Err("nope");

        let a: Result<()> = result.map_err(|e| {
            let mut r = ctx.report();
            r.err(span, e);
            r.err(other_pos, "previously reported here");
            r.into()
        });

        let e = a.unwrap_err();

        match e {
            ref e if e.message() == "Error in Context" => {}
            ref other => panic!("unexpected: {:?}", other),
        }

        assert_eq!(2, ctx.items().unwrap().len());
    }
}
