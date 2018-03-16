//! File spec collecting results from backends

use backend::IntoBytes;
use compiler::Compiler;
use core::errors::*;
use core::RpPackage;
use genco::{JavaScript, Tokens};

pub struct JsFileSpec<'el>(pub Tokens<'el, JavaScript<'el>>);

impl<'el> Default for JsFileSpec<'el> {
    fn default() -> Self {
        JsFileSpec(Tokens::new())
    }
}

impl<'el> IntoBytes<Compiler<'el>> for JsFileSpec<'el> {
    fn into_bytes(self, _: &Compiler<'el>, _: &RpPackage) -> Result<Vec<u8>> {
        let out = self.0.join_line_spacing().to_file()?;
        Ok(out.into_bytes())
    }
}
