//! Propagates scope-specific information to `into_model` transformations.

use core::{Context, RpName, RpPackage, RpVersionedPackage};
use core::errors::{Error, Result};
use naming::Naming;
use std::collections::HashMap;
use std::rc::Rc;

/// Root of the scope.
pub struct Root {
    ctx: Rc<Context>,
    package_prefix: Option<RpPackage>,
    package: RpVersionedPackage,
    prefixes: HashMap<String, RpVersionedPackage>,
    pub endpoint_naming: Option<Box<Naming>>,
    pub field_naming: Option<Box<Naming>>,
    keywords: Rc<HashMap<String, String>>,
    safe_packages: bool,
    package_naming: Option<Box<Naming>>,
    field_ident_naming: Option<Box<Naming>>,
}

impl Root {
    /// Rename the package, if applicable.
    pub fn package(&self, package: RpVersionedPackage) -> RpVersionedPackage {
        let package = if self.safe_packages {
            package.with_replacements(&self.keywords)
        } else {
            package
        };

        let package = if let Some(ref naming) = self.package_naming {
            package.with_naming(|p| naming.convert(p))
        } else {
            package
        };

        package
    }
}

/// Model of a scope.
enum Inner {
    Root(Rc<Root>),
    Child {
        root: Rc<Root>,
        name: String,
        parent: Rc<Inner>,
    },
}

pub struct Scope(Rc<Inner>);

impl Scope {
    pub fn new(
        ctx: Rc<Context>,
        package_prefix: Option<RpPackage>,
        package: RpVersionedPackage,
        prefixes: HashMap<String, RpVersionedPackage>,
        keywords: Rc<HashMap<String, String>>,
        safe_packages: bool,
        package_naming: Option<Box<Naming>>,
        field_ident_naming: Option<Box<Naming>>,
    ) -> Scope {
        let root = Rc::new(Root {
            ctx,
            package_prefix,
            package,
            prefixes,
            endpoint_naming: None,
            field_naming: None,
            keywords,
            safe_packages,
            package_naming,
            field_ident_naming,
        });

        Scope(Rc::new(Inner::Root(root)))
    }

    #[inline(always)]
    pub fn mut_root(&mut self) -> Result<&mut Root> {
        let inner = Rc::get_mut(&mut self.0).ok_or_else(|| Error::from("not uniquely owned"))?;

        match inner {
            &mut Inner::Root(ref mut root) => {
                Rc::get_mut(root).ok_or_else(|| Error::from("not uniquely owned"))
            }
            _ => return Err("scope is not a root element".into()),
        }
    }

    #[inline(always)]
    fn root(&self) -> &Rc<Root> {
        match *self.0 {
            Inner::Root(ref root) | Inner::Child { ref root, .. } => root,
        }
    }

    /// Walk the entire path of the scope.
    pub fn walk(&self) -> ScopeWalker {
        ScopeWalker {
            current: self.0.clone(),
        }
    }

    /// Create a new child scope.
    pub fn child<S: AsRef<str>>(&self, name: S) -> Scope {
        Scope(Rc::new(Inner::Child {
            root: self.root().clone(),
            name: name.as_ref().to_owned(),
            parent: self.0.clone(),
        }))
    }

    /// Access the error context.
    pub fn ctx(&self) -> &Context {
        self.root().ctx.as_ref()
    }

    /// Lookup what package a given prefix belongs to.
    pub fn lookup_prefix(&self, prefix: &String) -> Option<RpVersionedPackage> {
        let root = self.root();

        root.prefixes.get(prefix).map(|p| root.package(p.clone()))
    }

    /// Get the package that this scope belongs to.
    pub fn package(&self) -> RpVersionedPackage {
        let root = self.root();

        let package = root.package_prefix
            .as_ref()
            .map(|prefix| prefix.join_versioned(&root.package))
            .unwrap_or_else(|| root.package.clone());

        root.package(package)
    }

    /// Get the current path as a name.
    pub fn as_name(&self) -> RpName {
        let mut parts: Vec<_> = self.walk().collect();
        parts.reverse();

        RpName {
            prefix: None,
            package: self.package(),
            parts: parts,
        }
    }

    /// Access active endpoint naming.
    pub fn endpoint_naming(&self) -> Option<&Naming> {
        self.root().endpoint_naming.as_ref().map(AsRef::as_ref)
    }

    /// Access active field naming.
    pub fn field_naming(&self) -> Option<&Naming> {
        self.root().field_naming.as_ref().map(AsRef::as_ref)
    }

    /// Access field identifier naming.
    pub fn field_ident_naming(&self) -> Option<&Naming> {
        self.root().field_ident_naming.as_ref().map(AsRef::as_ref)
    }

    /// Lookup if the given identifier matches a language keyword.
    pub fn keyword(&self, identifier: &str) -> Option<&str> {
        self.root().keywords.get(identifier).map(|s| s.as_str())
    }
}

/// Walker over all components of the scope.
pub struct ScopeWalker {
    current: Rc<Inner>,
}

impl Iterator for ScopeWalker {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        let (next, current) = match *self.current {
            Inner::Root(_) => {
                return None;
            }
            Inner::Child {
                ref name,
                ref parent,
                ..
            } => (Some(name.to_owned()), parent.clone()),
        };

        self.current = current;
        next
    }
}

#[cfg(test)]
mod tests {
    use core::{RpPackage, RpVersionedPackage};
    use core::CapturingFilesystem;
    use core::Context;
    use scope::Scope;
    use std::collections::HashMap;
    use std::rc::Rc;

    #[test]
    pub fn test_scope() {
        let ctx = Rc::new(Context::new(Box::new(CapturingFilesystem::new())));
        let package = RpVersionedPackage::new(RpPackage::empty(), None);
        let prefixes = HashMap::new();
        let keywords = Rc::new(HashMap::new());
        let s = Scope::new(ctx, None, package, prefixes, keywords, false, None, None);

        let s2 = s.child("foo");
        let s3 = s2.child("bar");

        let parts: Vec<_> = s3.walk().collect();

        assert_eq!(vec!["bar".to_owned(), "foo".to_owned()], parts);
    }
}
