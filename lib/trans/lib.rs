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
    ($diag:expr, $attr:expr) => {{
        for unused in $attr.unused() {
            $diag.err(unused, "unknown attribute");
        }

        if $diag.has_errors() {
            return Err(());
        }
    }};
}

/// Helper macro to check that a selection has been completely consumed.
macro_rules! check_selection {
    ($diag:expr, $sel:expr) => {{
        for unused in $sel.unused() {
            $diag.err(unused, "unknown attribute");
        }

        if $diag.has_errors() {
            return Err(());
        }
    }};
}

mod attributes;
pub mod environment;
mod into_model;
mod scope;
pub mod translated;

pub use self::environment::{Environment, Packages};
pub use self::translated::Translated;
