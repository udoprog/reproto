use parser::ast;
use super::errors::*;
use super::into_model::IntoModel;
use super::rp_loc::{RpLoc, RpPos};
use super::rp_value::RpValue;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct RpFieldInit {
    pub name: RpLoc<String>,
    pub value: RpLoc<RpValue>,
}

impl IntoModel for ast::FieldInit {
    type Output = RpFieldInit;

    fn into_model(self, pos: &RpPos) -> Result<RpFieldInit> {
        let field_init = RpFieldInit {
            name: self.name.into_model(pos)?,
            value: self.value.into_model(pos)?,
        };

        Ok(field_init)
    }
}
