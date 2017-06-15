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
pub struct RpSubType {
    pub name: String,
    pub comment: Vec<String>,
    pub fields: Vec<RpLoc<RpField>>,
    pub codes: Vec<RpLoc<RpCode>>,
    pub names: Vec<RpLoc<String>>,
    pub match_decl: RpMatchDecl,
}

impl RpSubType {
    pub fn name(&self) -> String {
        self.names
            .iter()
            .map(|t| t.inner.to_owned())
            .nth(0)
            .unwrap_or_else(|| self.name.clone())
    }

    pub fn fields<'a>(&'a self) -> Box<Iterator<Item = &RpLoc<RpField>> + 'a> {
        Box::new(self.fields.iter())
    }
}

impl IntoModel for ast::SubType {
    type Output = Rc<RpSubType>;

    fn into_model(self, pos: &RpPos) -> Result<Rc<RpSubType>> {
        let mut fields: Vec<RpLoc<RpField>> = Vec::new();
        let mut codes = Vec::new();
        let mut options = Vec::new();
        let mut match_decl = RpMatchDecl::new();

        for member in self.members {
            let pos = (pos.0.to_owned(), member.pos.0, member.pos.1);

            match member.inner {
                ast::Member::Field(field) => {
                    let field = field.into_model(&pos)?;

                    if let Some(other) = fields.iter().find(|f| f.name == field.name) {
                        return Err(Error::field_conflict(field.name.clone(),
                                                         pos,
                                                         other.pos.clone()));
                    }

                    fields.push(RpLoc::new(field, pos));
                }
                ast::Member::Code(context, lines) => {
                    codes.push(utils::code(&pos, member.pos, context, lines));
                }
                ast::Member::Option(option) => {
                    options.push(option.into_model(&pos)?);
                }
                ast::Member::Match(m) => {
                    for member in m.members {
                        match_decl.push(member.into_model(&pos)?)?;
                    }
                }
            }
        }

        let options = Options::new(&pos, options);

        let names = options.find_all_strings("name")?;

        let sub_type = RpSubType {
            name: self.name,
            comment: self.comment,
            fields: fields,
            codes: codes,
            names: names,
            match_decl: match_decl,
        };

        Ok(Rc::new(sub_type))
    }
}

impl Merge for RpSubType {
    fn merge(&mut self, source: RpSubType) -> Result<()> {
        self.fields.merge(source.fields)?;
        self.codes.merge(source.codes)?;
        self.names.extend(source.names);
        Ok(())
    }
}
