/// Model of a scope.
pub enum Scope<'a> {
    Root,
    Child {
        name: &'a str,
        parent: &'a Scope<'a>,
    },
}

impl<'a> Scope<'a> {
    pub fn new() -> Scope<'a> {
        Scope::Root
    }

    pub fn child(&'a self, name: &'a str) -> Scope<'a> {
        Scope::Child {
            name: name,
            parent: self,
        }
    }

    pub fn walk(&self) -> ScopeWalker {
        ScopeWalker { current: self }
    }
}

pub struct ScopeWalker<'a> {
    current: &'a Scope<'a>,
}

impl<'a> Iterator for ScopeWalker<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        match self.current {
            &Scope::Root => None,
            &Scope::Child { name, parent } => {
                self.current = parent;
                Some(name)
            }
        }
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

        assert_eq!(vec!["bar", "foo"], parts);
    }
}
