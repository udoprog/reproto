//! Module that adds lombok annotations to generated classes.

use codegen::{ClassAdded, ClassCodegen, Configure};
use core::errors::*;
use genco::Java;
use genco::java::imported;

pub struct Module;

impl Module {
    pub fn initialize(self, e: Configure) {
        // lombok builds these automatically.
        e.options.build_getters = false;
        e.options.build_constructor = false;
        e.options.build_hash_code = false;
        e.options.build_equals = false;
        e.options.build_to_string = false;

        e.options.class_generators.push(Box::new(Lombok::new()));
    }
}

pub struct Lombok {
    data: Java<'static>,
}

impl Lombok {
    pub fn new() -> Lombok {
        Lombok { data: imported("lombok", "Data") }
    }
}

impl ClassCodegen for Lombok {
    fn generate(&self, e: ClassAdded) -> Result<()> {
        e.spec.annotation(toks!["@", self.data.clone()]);
        Ok(())
    }
}
