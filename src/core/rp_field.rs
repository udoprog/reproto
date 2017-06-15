use parser::ast;
use super::errors::*;
use super::into_model::IntoModel;
use super::merge::Merge;
use super::rp_loc::{RpLoc, RpPos};
use super::rp_modifier::RpModifier;
use super::rp_type::RpType;
use super::rp_value::RpValue;

#[derive(Debug, Clone, Serialize)]
pub struct RpField {
    pub modifier: RpModifier,
    pub name: String,
    pub comment: Vec<String>,
    pub ty: RpType,
    pub field_as: Option<RpLoc<String>>,
}

impl RpField {
    pub fn is_optional(&self) -> bool {
        match self.modifier {
            RpModifier::Optional => true,
            _ => false,
        }
    }

    pub fn name(&self) -> &str {
        if let Some(ref field) = self.field_as {
            &field.inner
        } else {
            &self.name
        }
    }

    pub fn display(&self) -> String {
        self.name.to_owned()
    }
}

impl IntoModel for ast::Field {
    type Output = RpField;

    fn into_model(self, pos: &RpPos) -> Result<RpField> {
        let field_as = self.field_as.into_model(pos)?;

        let field_as = if let Some(field_as) = field_as {
            if let RpValue::String(name) = field_as.inner {
                Some(RpLoc::new(name, field_as.pos.clone()))
            } else {
                return Err(Error::pos("must be a string".to_owned(), field_as.pos));
            }
        } else {
            None
        };

        let field = RpField {
            modifier: self.modifier,
            name: self.name,
            comment: self.comment,
            ty: self.ty,
            field_as: field_as,
        };

        Ok(field)
    }
}

impl Merge for Vec<RpLoc<RpField>> {
    fn merge(&mut self, source: Vec<RpLoc<RpField>>) -> Result<()> {
        for f in source {
            if let Some(field) = self.iter().find(|e| e.name == f.name) {
                return Err(Error::field_conflict(f.name.clone(), f.pos.clone(), field.pos.clone()));
            }

            self.push(f);
        }

        Ok(())
    }
}
