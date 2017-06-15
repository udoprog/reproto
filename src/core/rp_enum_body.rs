use parser::ast;
use std::rc::Rc;
use super::errors::*;
use super::into_model::IntoModel;
use super::merge::Merge;
use super::options::Options;
use super::rp_code::RpCode;
use super::rp_enum_variant::RpEnumVariant;
use super::rp_field::RpField;
use super::rp_loc::{RpLoc, RpPos};
use super::rp_match_decl::RpMatchDecl;
use super::utils;

#[derive(Debug, Clone, Serialize)]
pub struct RpEnumBody {
    pub name: String,
    pub comment: Vec<String>,
    pub variants: Vec<RpLoc<Rc<RpEnumVariant>>>,
    pub fields: Vec<RpLoc<RpField>>,
    pub codes: Vec<RpLoc<RpCode>>,
    pub match_decl: RpMatchDecl,
    pub serialized_as: Option<RpLoc<String>>,
    pub serialized_as_name: bool,
}

impl IntoModel for ast::EnumBody {
    type Output = Rc<RpEnumBody>;

    fn into_model(self, pos: &RpPos) -> Result<Rc<RpEnumBody>> {
        let mut variants: Vec<RpLoc<Rc<RpEnumVariant>>> = Vec::new();

        let mut ordinals = utils::OrdinalGenerator::new();

        let (fields, codes, options, match_decl) = utils::members_into_model(pos, self.members)?;

        for variant in self.variants {
            let ordinal = ordinals.next(&variant.ordinal, pos)
                .map_err(|e| Error::pos(e.description().into(), pos.clone()))?;

            let variant = RpLoc::new((variant.inner, ordinal).into_model(pos)?,
                                     (pos.0.clone(), variant.pos.0, variant.pos.1));

            if fields.len() != variant.arguments.len() {
                return Err(Error::pos(format!("expected {} arguments", fields.len()),
                                      variant.pos.clone()));
            }

            if let Some(other) = variants.iter().find(|v| *v.name == *variant.name) {
                return Err(ErrorKind::EnumVariantConflict(other.name.pos.clone(),
                                                          variant.name.pos.clone())
                    .into());
            }

            variants.push(variant);
        }

        let options = Options::new(pos, options);

        let serialized_as: Option<RpLoc<String>> = options.find_one_identifier("serialized_as")?
            .to_owned();

        let serialized_as_name = options.find_one_boolean("serialized_as_name")?
            .to_owned()
            .map(|t| t.inner)
            .unwrap_or(false);

        let en = RpEnumBody {
            name: self.name,
            comment: self.comment,
            variants: variants,
            fields: fields,
            codes: codes,
            match_decl: match_decl,
            serialized_as: serialized_as,
            serialized_as_name: serialized_as_name,
        };

        Ok(Rc::new(en))
    }
}

impl Merge for RpEnumBody {
    fn merge(&mut self, source: RpEnumBody) -> Result<()> {
        self.codes.merge(source.codes)?;
        Ok(())
    }
}
