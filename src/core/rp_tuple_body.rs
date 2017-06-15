use parser::ast;
use std::rc::Rc;
use super::errors::*;
use super::into_model::IntoModel;
use super::merge::Merge;
use super::options::Options;
use super::rp_code::RpCode;
use super::rp_field::RpField;
use super::rp_loc::{RpLoc, RpPos};
use super::rp_match_decl::RpMatchDecl;
use super::utils;

#[derive(Debug, Clone, Serialize)]
pub struct RpTupleBody {
    pub name: String,
    pub comment: Vec<String>,
    pub fields: Vec<RpLoc<RpField>>,
    pub codes: Vec<RpLoc<RpCode>>,
    pub match_decl: RpMatchDecl,
}

impl RpTupleBody {
    pub fn fields<'a>(&'a self) -> Box<Iterator<Item = &RpLoc<RpField>> + 'a> {
        Box::new(self.fields.iter())
    }
}

impl IntoModel for ast::TupleBody {
    type Output = Rc<RpTupleBody>;

    fn into_model(self, pos: &RpPos) -> Result<Rc<RpTupleBody>> {
        let (fields, codes, options, match_decl) = utils::members_into_model(&pos, self.members)?;

        let _options = Options::new(&pos, options);

        let tuple_body = RpTupleBody {
            name: self.name,
            comment: self.comment,
            fields: fields,
            codes: codes,
            match_decl: match_decl,
        };

        Ok(Rc::new(tuple_body))
    }
}

impl Merge for RpTupleBody {
    fn merge(&mut self, source: RpTupleBody) -> Result<()> {
        self.fields.merge(source.fields)?;
        self.codes.merge(source.codes)?;
        Ok(())
    }
}
