use parser::ast;
use std::rc::Rc;
use super::errors::*;
use super::into_model::IntoModel;
use super::merge::Merge;
use super::rp_enum_body::RpEnumBody;
use super::rp_interface_body::RpInterfaceBody;
use super::rp_loc::{RpLoc, RpPos};
use super::rp_tuple_body::RpTupleBody;
use super::rp_type_body::RpTypeBody;

#[derive(Clone)]
pub enum RpDecl {
    Type(Rc<RpTypeBody>),
    Interface(Rc<RpInterfaceBody>),
    Enum(Rc<RpEnumBody>),
    Tuple(Rc<RpTupleBody>),
}

impl RpDecl {
    pub fn name(&self) -> &str {
        match *self {
            RpDecl::Type(ref body) => &body.name,
            RpDecl::Interface(ref body) => &body.name,
            RpDecl::Enum(ref body) => &body.name,
            RpDecl::Tuple(ref body) => &body.name,
        }
    }
}

impl ::std::fmt::Display for RpDecl {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        match *self {
            RpDecl::Type(ref body) => write!(f, "type {}", body.name),
            RpDecl::Interface(ref body) => write!(f, "interface {}", body.name),
            RpDecl::Enum(ref body) => write!(f, "enum {}", body.name),
            RpDecl::Tuple(ref body) => write!(f, "tuple {}", body.name),
        }
    }
}

impl IntoModel for ast::Decl {
    type Output = RpDecl;

    fn into_model(self, pos: &RpPos) -> Result<RpDecl> {
        let decl = match self {
            ast::Decl::Type(body) => RpDecl::Type(body.into_model(pos)?),
            ast::Decl::Interface(body) => RpDecl::Interface(body.into_model(pos)?),
            ast::Decl::Enum(body) => RpDecl::Enum(body.into_model(pos)?),
            ast::Decl::Tuple(body) => RpDecl::Tuple(body.into_model(pos)?),
        };

        Ok(decl)
    }
}

impl Merge for RpLoc<RpDecl> {
    fn merge(&mut self, source: RpLoc<RpDecl>) -> Result<()> {
        let dest_pos = self.pos.clone();

        match self.inner {
            RpDecl::Type(ref mut body) => {
                if let RpDecl::Type(other) = source.inner {
                    return body.merge(other);
                }
            }
            RpDecl::Enum(ref mut body) => {
                if let RpDecl::Enum(other) = source.inner {
                    if let Some(variant) = other.variants.iter().next() {
                        return Err(Error::extend_enum("cannot extend enum with additional \
                                                       variants"
                                                          .to_owned(),
                                                      variant.pos.clone(),
                                                      dest_pos));
                    }

                    if let Some(field) = other.fields.iter().next() {
                        return Err(Error::extend_enum("cannot extend enum with additional fields"
                                                          .to_owned(),
                                                      field.pos.clone(),
                                                      dest_pos));
                    }


                    return body.merge(other);
                }
            }
            RpDecl::Interface(ref mut body) => {
                if let RpDecl::Interface(other) = source.inner {
                    return body.merge(other);
                }
            }
            RpDecl::Tuple(ref mut body) => {
                if let RpDecl::Tuple(other) = source.inner {
                    return body.merge(other);
                }
            }
        }

        return Err(Error::decl_merge(format!("cannot merge with {}", source),
                                     source.pos,
                                     dest_pos));
    }
}
