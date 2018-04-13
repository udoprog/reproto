//! The file spec collecting changes.

use backend::IntoBytes;
use compiler::Compiler;
use core::RpPackage;
use core::errors::Result;
use genco::{Rust, Tokens};

pub struct RustFileSpec<'a>(pub Tokens<'a, Rust<'a>>);

impl<'el> Default for RustFileSpec<'el> {
    fn default() -> Self {
        RustFileSpec(Tokens::new())
    }
}

impl<'el> IntoBytes<Compiler<'el>> for RustFileSpec<'el> {
    fn into_bytes(self, _: &Compiler<'el>, _: &RpPackage) -> Result<Vec<u8>> {
        let out = self.0.join_line_spacing().to_file()?;
        Ok(out.into_bytes())
    }
}
