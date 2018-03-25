//! File declarations

use errors::Result;
use std::collections::LinkedList;
use {Flavor, RpDecl, Translate, Translator};

#[derive(Debug, Clone, Serialize)]
pub struct RpFile<F: 'static>
where
    F: Flavor,
{
    pub comment: Vec<String>,
    pub decls: Vec<RpDecl<F>>,
}

/// Iterator over all declarations in a file.
#[allow(linkedlist)]
pub struct ForEachDecl<'a, F: 'static>
where
    F: Flavor,
{
    queue: LinkedList<&'a RpDecl<F>>,
}

impl<'a, F: 'static> Iterator for ForEachDecl<'a, F>
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

impl<F: 'static> RpFile<F>
where
    F: Flavor,
{
    /// Iterate over all declarations in file.
    pub fn for_each_decl(&self) -> ForEachDecl<F> {
        let mut queue = LinkedList::new();
        queue.extend(self.decls.iter());
        ForEachDecl { queue: queue }
    }
}

impl<F: 'static, T> Translate<T> for RpFile<F>
where
    F: Flavor,
    T: Translator<Source = F>,
{
    type Source = F;
    type Out = RpFile<T::Target>;

    /// Translate into different flavor.
    fn translate(self, translator: &T) -> Result<RpFile<T::Target>> {
        Ok(RpFile {
            comment: self.comment,
            decls: self.decls.translate(translator)?,
        })
    }
}
