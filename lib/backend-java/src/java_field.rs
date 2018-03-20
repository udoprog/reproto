pub use super::*;
use genco::Cons;
use genco::java::{Argument, Field, Method, Modifier};
use std::rc::Rc;

/// A single field.
#[derive(Debug, Clone)]
pub struct JavaField<'el> {
    pub name: Cons<'el>,
    pub ident: Rc<String>,
    pub field_accessor: Rc<String>,
    pub spec: Field<'el>,
}

impl<'el> JavaField<'el> {
    pub fn setter(&self) -> Option<Method<'el>> {
        if self.spec.modifiers.contains(&Modifier::Final) {
            return None;
        }

        let argument = Argument::new(self.spec.ty(), self.spec.var());
        let mut m = Method::new(Rc::new(format!("set{}", self.field_accessor)));

        m.arguments.push(argument.clone());

        m.body.push(
            toks!["this.", self.spec.var(), " = ", argument.var(), ";",],
        );

        Some(m)
    }

    /// Create a new getter method without a body.
    pub fn getter_without_body(&self) -> Method<'el> {
        // Avoid `getClass`, a common built-in method for any Object.
        let field_accessor = match self.field_accessor.as_str() {
            "Class" => "Class_",
            accessor => accessor,
        };

        let mut method = Method::new(Rc::new(format!("get{}", field_accessor)));
        method.comments = self.spec.comments.clone();
        method.returns = self.spec.ty().as_field();
        method
    }

    /// Build a new complete getter.
    pub fn getter(&self) -> Method<'el> {
        let mut m = self.getter_without_body();
        m.body.push(toks!["return this.", self.spec.var(), ";"]);
        m
    }

    /// The JSON name of the field.
    pub fn name(&self) -> Cons<'el> {
        self.name.clone()
    }
}
