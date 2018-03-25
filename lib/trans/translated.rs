use core::errors::Result;
use core::{Flavor, RpDecl, RpFile, RpName, RpReg, RpVersionedPackage};
use linked_hash_map::LinkedHashMap;
use std::collections::{btree_map, BTreeMap, LinkedList};
use std::vec;

/// Iterate over all files in the environment.
pub struct ForEachFile<'a, F: 'static>
where
    F: Flavor,
{
    iter: btree_map::Iter<'a, RpVersionedPackage, RpFile<F>>,
}

impl<'a, F: 'static> Iterator for ForEachFile<'a, F>
where
    F: Flavor,
{
    type Item = (&'a RpVersionedPackage, &'a RpFile<F>);

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

/// Iterator over all toplevel declarations.
pub struct ToplevelDeclIter<'a, F: 'static>
where
    F: Flavor,
{
    it: vec::IntoIter<&'a RpDecl<F>>,
}

impl<'a, F: 'static> Iterator for ToplevelDeclIter<'a, F>
where
    F: Flavor,
{
    type Item = &'a RpDecl<F>;

    fn next(&mut self) -> Option<Self::Item> {
        self.it.next()
    }
}

/// Iterator over all declarations in a file.
pub struct DeclIter<'a, F: 'static>
where
    F: Flavor,
{
    queue: LinkedList<&'a RpDecl<F>>,
}

impl<'a, F: 'static> Iterator for DeclIter<'a, F>
where
    F: Flavor,
{
    type Item = &'a RpDecl<F>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(decl) = self.queue.pop_front() {
            self.queue.extend(decl.decls());
            Some(decl)
        } else {
            None
        }
    }
}

/// An environment that has been translated into a target environment.
pub struct Translated<F: 'static>
where
    F: Flavor,
{
    /// Registered types.
    types: LinkedHashMap<RpName, RpReg>,
    /// Files and associated declarations.
    files: BTreeMap<RpVersionedPackage, RpFile<F>>,
}

impl<F: 'static> Translated<F>
where
    F: Flavor,
{
    pub fn new(
        types: LinkedHashMap<RpName, RpReg>,
        files: BTreeMap<RpVersionedPackage, RpFile<F>>,
    ) -> Self {
        Self {
            types: types,
            files: files,
        }
    }

    /// Lookup the declaration matching the given name.
    ///
    /// Returns the registered reference, if present.
    pub fn lookup<'a>(&'a self, name: &RpName) -> Result<&'a RpReg> {
        let key = name.clone().without_prefix();

        if let Some(registered) = self.types.get(&key) {
            return Ok(registered);
        }

        return Err(format!("no such type: {}", name).into());
    }

    /// Iterate over all files.
    pub fn for_each_file(&self) -> ForEachFile<F> {
        ForEachFile {
            iter: self.files.iter(),
        }
    }

    /// Iterate over top level declarations of all registered objects.
    pub fn toplevel_decl_iter(&self) -> ToplevelDeclIter<F> {
        let values = self.files
            .values()
            .flat_map(|f| f.decls.iter())
            .collect::<Vec<_>>();

        ToplevelDeclIter {
            it: values.into_iter(),
        }
    }

    /// Walks the entire tree of declarations recursively of all registered objects.
    pub fn decl_iter(&self) -> DeclIter<F> {
        let mut queue = LinkedList::new();
        queue.extend(self.files.values().flat_map(|f| f.decls.iter()));
        DeclIter { queue: queue }
    }
}
