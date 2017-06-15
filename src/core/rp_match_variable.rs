use parser::ast;
use super::errors::*;
use super::into_model::IntoModel;
use super::rp_loc::RpPos;
use super::rp_type::RpType;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RpMatchVariable {
    pub name: String,
    pub ty: RpType,
}

impl IntoModel for ast::MatchVariable {
    type Output = RpMatchVariable;

    fn into_model(self, pos: &RpPos) -> Result<RpMatchVariable> {
        let match_variable = RpMatchVariable {
            name: self.name.into_model(pos)?,
            ty: self.ty,
        };

        Ok(match_variable)
    }
}
