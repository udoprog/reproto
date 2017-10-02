use super::*;

#[derive(Clone)]
pub struct PythonField<'a> {
    pub modifier: &'a RpModifier,
    pub ty: &'a RpType,
    pub name: &'a str,
    pub ident: String,
}

impl<'a> PythonField<'a> {
    pub fn with_ident(self, ident: String) -> PythonField<'a> {
        PythonField {
            modifier: self.modifier,
            ty: self.ty,
            name: self.name,
            ident: ident,
        }
    }
}
