use std::rc::Rc;

/// Model of a scope.
enum Inner {
    Root,
    Child { name: String, parent: Rc<Inner> },
}

pub struct Scope {
    inner: Rc<Inner>,
}

impl Scope {
    pub fn new() -> Scope {
        Scope { inner: Rc::new(Inner::Root) }
    }

    pub fn child<S: AsRef<str>>(&self, name: S) -> Scope {
        Scope {
            inner: Rc::new(Inner::Child {
                name: name.as_ref().to_owned(),
                parent: self.inner.clone(),
            }),
        }
    }

    pub fn walk(&self) -> ScopeWalker {
        ScopeWalker { current: self.inner.clone() }
    }
}

pub struct ScopeWalker {
    current: Rc<Inner>,
}

impl Iterator for ScopeWalker {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        use self::Inner::*;

        let (next, current) = match *self.current {
            Root => {
                return None;
            }
            Child {
                ref name,
                ref parent,
            } => (Some(name.to_owned()), parent.clone()),
        };

        self.current = current;
        next
    }
}

#[cfg(test)]
mod tests {
    use super::Scope;

    #[test]
    pub fn test_scope() {
        let s = Scope::new();
        let s2 = s.child("foo");
        let s3 = s2.child("bar");

        let parts: Vec<_> = s3.walk().collect();

        assert_eq!(vec!["bar".to_owned(), "foo".to_owned()], parts);
    }
}
