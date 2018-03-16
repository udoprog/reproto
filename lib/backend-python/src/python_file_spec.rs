use backend::IntoBytes;
use compiler::Compiler;
use core::errors::Result;
use core::RpPackage;
use genco::{Python, Tokens};

pub struct PythonFileSpec<'el>(pub Tokens<'el, Python<'el>>);

impl<'el> Default for PythonFileSpec<'el> {
    fn default() -> Self {
        PythonFileSpec(Tokens::new())
    }
}

impl<'el> IntoBytes<Compiler<'el>> for PythonFileSpec<'el> {
    fn into_bytes(self, _: &Compiler<'el>, _: &RpPackage) -> Result<Vec<u8>> {
        let out = self.0.join_line_spacing().to_file()?;
        Ok(out.into_bytes())
    }
}
