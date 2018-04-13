//! gRPC module for Rust.

use Options;
use backend::Initializer;

pub struct Module {}

impl Module {
    pub fn new() -> Module {
        Module {}
    }
}

impl Initializer for Module {
    type Options = Options;
}
