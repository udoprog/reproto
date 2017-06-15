use parser::ast;
use std::collections::{BTreeMap, btree_map};
use std::rc::Rc;
use super::errors::*;
use super::into_model::IntoModel;
use super::merge::Merge;
use super::options::Options;
use super::rp_code::RpCode;
use super::rp_field::RpField;
use super::rp_loc::{RpLoc, RpPos};
use super::rp_match_decl::RpMatchDecl;
use super::rp_sub_type::RpSubType;
use super::utils;

#[derive(Debug, Clone, Serialize)]
pub struct RpInterfaceBody {
    pub name: String,
    pub comment: Vec<String>,
    pub fields: Vec<RpLoc<RpField>>,
    pub codes: Vec<RpLoc<RpCode>>,
    pub match_decl: RpMatchDecl,
    pub sub_types: BTreeMap<String, RpLoc<Rc<RpSubType>>>,
}

impl RpInterfaceBody {
    pub fn fields<'a>(&'a self) -> Box<Iterator<Item = &RpLoc<RpField>> + 'a> {
        Box::new(self.fields.iter())
    }
}

impl IntoModel for ast::InterfaceBody {
    type Output = Rc<RpInterfaceBody>;

    fn into_model(self, pos: &RpPos) -> Result<Rc<RpInterfaceBody>> {
        let (fields, codes, options, match_decl) = utils::members_into_model(&pos, self.members)?;

        let mut sub_types: BTreeMap<String, RpLoc<Rc<RpSubType>>> = BTreeMap::new();

        for sub_type in self.sub_types.into_model(pos)? {
            // key has to be owned by entry
            let key = sub_type.name.clone();

            match sub_types.entry(key) {
                btree_map::Entry::Occupied(entry) => {
                    entry.into_mut().merge(sub_type)?;
                }
                btree_map::Entry::Vacant(entry) => {
                    entry.insert(sub_type);
                }
            }
        }

        let _options = Options::new(&pos, options);

        let interface_body = RpInterfaceBody {
            name: self.name,
            comment: self.comment,
            fields: fields,
            codes: codes,
            match_decl: match_decl,
            sub_types: sub_types,
        };

        Ok(Rc::new(interface_body))
    }
}

impl Merge for RpInterfaceBody {
    fn merge(&mut self, source: RpInterfaceBody) -> Result<()> {
        self.fields.merge(source.fields)?;
        self.codes.merge(source.codes)?;
        self.sub_types.merge(source.sub_types)?;
        Ok(())
    }
}
