//! Module that adds the @ConstructorProperties annotation to every constructor.

use codegen::{ClassAdded, ClassCodegen, Configure};
use core::errors::Result;
use genco::{Java, Quoted, Tokens};
use genco::java::imported;

pub struct Module;

impl Module {
    pub fn initialize(self, e: Configure) {
        e.options.class_generators.push(Box::new(
            ConstructorProperties::new(),
        ));
    }
}

pub struct ConstructorProperties {
    annotation: Java<'static>,
}

impl ConstructorProperties {
    pub fn new() -> ConstructorProperties {
        ConstructorProperties { annotation: imported("java.beans", "ConstructorProperties") }
    }
}

impl ClassCodegen for ConstructorProperties {
    fn generate(&self, e: ClassAdded) -> Result<()> {
        let args: Tokens<Java> = e.names.iter().cloned().map(Quoted::quoted).collect();
        let a = toks![self.annotation.clone(), "({", args.join(", "), "})"];

        for c in &mut e.spec.constructors {
            c.annotation(a.clone());
        }

        Ok(())
    }
}
