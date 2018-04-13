pub use super::*;
use genco::Cons;
use genco::csharp::Field;
use std::rc::Rc;

/// A single field.
#[derive(Debug, Clone)]
pub struct CsharpField<'el> {
    pub name: Cons<'el>,
    pub ident: Rc<String>,
    pub spec: Field<'el>,
    pub optional: bool,
}

impl<'el> CsharpField<'el> {
    /// The JSON name of the field.
    pub fn name(&self) -> Cons<'el> {
        self.name.clone()
    }
}
