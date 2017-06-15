use parser::ast;
use super::errors::*;
use super::into_model::IntoModel;
use super::rp_field_init::RpFieldInit;
use super::rp_loc::{RpLoc, RpPos};
use super::rp_name::RpName;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct RpInstance {
    pub ty: RpName,
    pub arguments: RpLoc<Vec<RpLoc<RpFieldInit>>>,
}

impl IntoModel for ast::Instance {
    type Output = RpInstance;

    fn into_model(self, pos: &RpPos) -> Result<RpInstance> {
        let instance = RpInstance {
            ty: self.ty,
            arguments: self.arguments.into_model(pos)?,
        };

        Ok(instance)
    }
}
