use parser::ast;
use std::rc::Rc;
use super::errors::*;
use super::into_model::IntoModel;
use super::rp_loc::{RpLoc, RpPos};
use super::rp_value::RpValue;

#[derive(Debug, Clone, Serialize)]
pub struct RpEnumVariant {
    pub name: RpLoc<String>,
    pub comment: Vec<String>,
    pub arguments: Vec<RpLoc<RpValue>>,
    pub ordinal: u32,
}

/// enum value with assigned ordinal
impl IntoModel for (ast::EnumVariant, u32) {
    type Output = Rc<RpEnumVariant>;

    fn into_model(self, pos: &RpPos) -> Result<Self::Output> {
        let value = self.0;
        let ordinal = self.1;

        let value = RpEnumVariant {
            name: value.name.into_model(pos)?,
            comment: value.comment,
            arguments: value.arguments.into_model(pos)?,
            ordinal: ordinal,
        };

        Ok(Rc::new(value))
    }
}
