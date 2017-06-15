use backend::errors::*;
use parser::ast;
use std::collections::HashSet;
use std::rc::Rc;
use super::into_model::IntoModel;
use super::merge::Merge;
use super::options::Options;
use super::rp_code::RpCode;
use super::rp_field::RpField;
use super::rp_loc::{RpLoc, RpPos};
use super::rp_match_decl::RpMatchDecl;
use super::utils;

#[derive(Debug, Clone, Serialize)]
pub struct RpTypeBody {
    pub name: String,
    pub comment: Vec<String>,
    pub fields: Vec<RpLoc<RpField>>,
    pub codes: Vec<RpLoc<RpCode>>,
    pub match_decl: RpMatchDecl,
    // Set of fields which are reserved for this type.
    pub reserved: HashSet<RpLoc<String>>,
}

impl RpTypeBody {
    pub fn verify(&self) -> Result<()> {
        for reserved in &self.reserved {
            if let Some(field) = self.fields.iter().find(|f| f.name == reserved.inner) {
                return Err(Error::reserved_field(field.pos.clone(), reserved.pos.clone()));
            }
        }

        Ok(())
    }

    pub fn fields<'a>(&'a self) -> Box<Iterator<Item = &RpLoc<RpField>> + 'a> {
        Box::new(self.fields.iter())
    }
}

impl IntoModel for ast::TypeBody {
    type Output = Rc<RpTypeBody>;

    fn into_model(self, pos: &RpPos) -> Result<Rc<RpTypeBody>> {
        let (fields, codes, options, match_decl) = utils::members_into_model(&pos, self.members)?;

        let options = Options::new(&pos, options);

        let reserved: HashSet<RpLoc<String>> =
            options.find_all_identifiers("reserved")?.into_iter().collect();

        let type_body = RpTypeBody {
            name: self.name,
            comment: self.comment,
            fields: fields,
            codes: codes,
            match_decl: match_decl,
            reserved: reserved,
        };

        Ok(Rc::new(type_body))
    }
}

impl Merge for RpTypeBody {
    fn merge(&mut self, source: RpTypeBody) -> Result<()> {
        self.fields.merge(source.fields)?;
        self.codes.merge(source.codes)?;
        Ok(())
    }
}
