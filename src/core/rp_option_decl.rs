use parser::ast;
use super::errors::*;
use super::into_model::IntoModel;
use super::rp_loc::{RpLoc, RpPos};
use super::rp_value::RpValue;

#[derive(Debug, Serialize)]
pub struct RpOptionDecl {
    pub name: String,
    pub values: Vec<RpLoc<RpValue>>,
}

impl IntoModel for ast::OptionDecl {
    type Output = RpOptionDecl;

    fn into_model(self, pos: &RpPos) -> Result<RpOptionDecl> {
        let decl = RpOptionDecl {
            name: self.name,
            values: self.values.into_model(pos)?,
        };

        Ok(decl)
    }
}
