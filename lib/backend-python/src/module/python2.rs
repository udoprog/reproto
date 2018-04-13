//! Module that adds fasterxml annotations to generated classes.

use Options;
use backend::Initializer;
use core::errors::Result;
use genco::{Cons, Python, Tokens};
use std::rc::Rc;
use utils::VersionHelper;

#[derive(Debug, Default, Deserialize)]
pub struct Config {}

pub struct Module {
    #[allow(dead_code)]
    config: Config,
}

impl Module {
    pub fn new(config: Config) -> Module {
        Module { config: config }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Python2VersionHelper {}

impl VersionHelper for Python2VersionHelper {
    fn is_string<'el>(&self, var: Cons<'el>) -> Tokens<'el, Python<'el>> {
        toks!["isinstance(", var, ", unicode)"]
    }
}

impl Initializer for Module {
    type Options = Options;

    fn initialize(&self, options: &mut Options) -> Result<()> {
        options.version_helper = Rc::new(Box::new(Python2VersionHelper {}));

        Ok(())
    }
}
