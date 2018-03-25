extern crate linked_hash_map;
#[macro_use]
extern crate log;
extern crate reproto_ast as ast;
extern crate reproto_core as core;
extern crate reproto_naming as naming;
extern crate reproto_parser as parser;
extern crate reproto_path_parser as path_parser;

/// Helper macro to check that an attribute has been completely consumed.
macro_rules! check_attributes {
    ($ctx: expr, $attr: expr) => {{
        let mut __a_r = $ctx.report();

        for unused in $attr.unused() {
            __a_r = __a_r.err(unused, "unknown attribute");
        }

        if let Some(e) = __a_r.close() {
            return Err(e.into());
        }
    }};
}

/// Helper macro to check that a selection has been completely consumed.
macro_rules! check_selection {
    ($ctx: expr, $sel: expr) => {{
        let mut __a_r = $ctx.report();

        for unused in $sel.unused() {
            __a_r = __a_r.err(unused, "unknown attribute");
        }

        if let Some(e) = __a_r.close() {
            return Err(e.into());
        }
    }};
}

mod attributes;
pub mod environment;
mod into_model;
mod scope;
pub mod translated;

pub use self::environment::Environment;
pub use self::translated::Translated;
