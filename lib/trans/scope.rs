//! Propagates scope-specific information to `into_model` transformations.

use core::errors::Error;
use core::{CoreFlavor, Import, Loc, RpName, RpRequiredPackage, RpVersionedPackage, Span};
use naming::Naming;
use std::collections::HashMap;
use std::rc::Rc;

pub struct Scope<I> {
    package: RpVersionedPackage,
    /// Language keywords to avoid.
    keywords: Rc<HashMap<String, String>>,
    field_ident_naming: Option<Box<Naming>>,
    endpoint_ident_naming: Option<Box<Naming>>,
    import: I,
    pub endpoint_naming: Option<Box<Naming>>,
    pub field_naming: Option<Box<Naming>>,
    pub prefixes: HashMap<String, RpVersionedPackage>,
    /// Path of the current scope.
    path: Vec<String>,
}

impl<I> Scope<I> {
    pub fn new(
        package: RpVersionedPackage,
        keywords: Rc<HashMap<String, String>>,
        field_ident_naming: Option<Box<Naming>>,
        endpoint_ident_naming: Option<Box<Naming>>,
        import: I,
    ) -> Scope<I> {
        Self {
            package,
            keywords,
            field_ident_naming,
            endpoint_ident_naming,
            import,
            endpoint_naming: None,
            field_naming: None,
            prefixes: HashMap::new(),
            path: vec![],
        }
    }

    /// Create a new child scope.
    pub fn push<S: AsRef<str>>(&mut self, name: S) {
        self.path.push(name.as_ref().to_string());
    }

    /// Pop the last name component.
    pub fn pop(&mut self) {
        self.path.pop();
    }

    /// Lookup what package a given prefix belongs to.
    pub fn lookup_prefix(&self, prefix: &str) -> Option<RpVersionedPackage> {
        self.prefixes.get(prefix).map(Clone::clone)
    }

    /// Get the package that this scope belongs to.
    pub fn package(&self) -> RpVersionedPackage {
        self.package.clone()
    }

    /// Get the current path as a name.
    pub fn as_name(&self, span: Span) -> Loc<RpName<CoreFlavor>> {
        Loc::new(
            RpName {
                prefix: None,
                package: self.package(),
                parts: self.path.clone(),
            },
            span,
        )
    }

    /// Access active endpoint naming.
    pub fn endpoint_naming(&self) -> Option<&Naming> {
        self.endpoint_naming.as_ref().map(AsRef::as_ref)
    }

    /// Access active field naming.
    pub fn field_naming(&self) -> Option<&Naming> {
        self.field_naming.as_ref().map(AsRef::as_ref)
    }

    /// Access field identifier naming.
    pub fn field_ident_naming(&self) -> Option<&Naming> {
        self.field_ident_naming.as_ref().map(AsRef::as_ref)
    }

    /// Access endpoint identifier naming.
    pub fn endpoint_ident_naming(&self) -> Option<&Naming> {
        self.endpoint_ident_naming.as_ref().map(AsRef::as_ref)
    }

    /// Lookup if the given identifier matches a language keyword.
    pub fn keyword(&self, identifier: &str) -> Option<&str> {
        self.keywords.get(identifier).map(|s| s.as_str())
    }
}

impl<I> Scope<I>
where
    I: Import,
{
    pub fn import(
        &mut self,
        package: &RpRequiredPackage,
    ) -> Result<Option<RpVersionedPackage>, Error> {
        self.import.import(package)
    }
}

#[cfg(test)]
mod tests {
    use core::{RpPackage, RpVersionedPackage};
    use scope::Scope;
    use std::collections::HashMap;
    use std::rc::Rc;

    #[test]
    pub fn test_scope() {
        let package = RpVersionedPackage::new(RpPackage::empty(), None);
        let keywords = Rc::new(HashMap::new());

        let s = Scope::new(package, keywords, None, None);

        let s2 = s.child("foo");
        let s3 = s2.child("bar");

        let parts: Vec<_> = s3.walk().collect();

        assert_eq!(vec!["bar".to_owned(), "foo".to_owned()], parts);
    }
}
