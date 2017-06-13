use std::rc::Rc;
use super::errors::*;
use super::rp_enum_body::RpEnumBody;
use super::rp_enum_variant::RpEnumVariant;
use super::rp_field::RpField;
use super::rp_interface_body::RpInterfaceBody;
use super::rp_loc::RpLoc;
use super::rp_sub_type::RpSubType;
use super::rp_tuple_body::RpTupleBody;
use super::rp_type_body::RpTypeBody;

#[derive(Debug, Clone)]
pub enum RpRegistered {
    Type(Rc<RpTypeBody>),
    Interface(Rc<RpInterfaceBody>),
    Enum(Rc<RpEnumBody>),
    Tuple(Rc<RpTupleBody>),
    SubType {
        parent: Rc<RpInterfaceBody>,
        sub_type: Rc<RpSubType>,
    },
    EnumConstant {
        parent: Rc<RpEnumBody>,
        variant: Rc<RpEnumVariant>,
    },
}

impl RpRegistered {
    pub fn fields<'a>(&'a self) -> Result<Box<Iterator<Item = &RpLoc<RpField>> + 'a>> {
        let it: Box<Iterator<Item = &RpLoc<RpField>>> = match *self {
            RpRegistered::Type(ref body) => Box::new(body.fields.iter()),
            RpRegistered::Tuple(ref body) => Box::new(body.fields.iter()),
            RpRegistered::SubType { ref parent, ref sub_type } => {
                Box::new(parent.fields.iter().chain(sub_type.fields.iter()))
            }
            _ => {
                return Err("has no fields".into());
            }
        };

        Ok(it)
    }

    pub fn find_field(&self, name: &str) -> Result<Option<&RpLoc<RpField>>> {
        for field in self.fields()? {
            if field.name == name {
                return Ok(Some(field));
            }
        }

        Ok(None)
    }

    pub fn is_assignable_from(&self, other: &RpRegistered) -> bool {
        match (self, other) {
            // exact type
            (&RpRegistered::Type(ref target), &RpRegistered::Type(ref source)) => {
                Rc::ptr_eq(target, source)
            }
            // exact tuple
            (&RpRegistered::Tuple(ref target), &RpRegistered::Tuple(ref source)) => {
                Rc::ptr_eq(target, source)
            }
            // exact interface, with unknown sub-type.
            (&RpRegistered::Interface(ref target), &RpRegistered::Interface(ref source)) => {
                Rc::ptr_eq(target, source)
            }
            // exact enum, with unknown value
            (&RpRegistered::Enum(ref target), &RpRegistered::Enum(ref source)) => {
                Rc::ptr_eq(target, source)
            }
            // sub-type to parent
            (&RpRegistered::Interface(ref target),
             &RpRegistered::SubType { parent: ref source, sub_type: _ }) => {
                Rc::ptr_eq(target, source)
            }
            // enum constant to parent type
            (&RpRegistered::Enum(ref target),
             &RpRegistered::EnumConstant { parent: ref source, variant: _ }) => {
                Rc::ptr_eq(target, source)
            }
            // exact matching sub-type
            (&RpRegistered::SubType { parent: ref target_parent, sub_type: ref target },
             &RpRegistered::SubType { parent: ref source_parent, sub_type: ref source }) => {
                Rc::ptr_eq(target_parent, source_parent) && Rc::ptr_eq(target, source)
            }
            // exact matching constant
            (&RpRegistered::EnumConstant { parent: ref target_parent, variant: ref target },
             &RpRegistered::EnumConstant { parent: ref source_parent, variant: ref source }) => {
                Rc::ptr_eq(target_parent, source_parent) && Rc::ptr_eq(target, source)
            }
            _ => false,
        }
    }

    pub fn display(&self) -> String {
        match *self {
            RpRegistered::Type(ref body) => format!("type {}", body.name.to_owned()),
            RpRegistered::Interface(ref body) => format!("interface {}", body.name.to_owned()),
            RpRegistered::Enum(ref body) => format!("enum {}", body.name.to_owned()),
            RpRegistered::Tuple(ref body) => format!("tuple {}", body.name.to_owned()),
            RpRegistered::SubType { ref parent, ref sub_type } => {
                format!("type {}.{}", parent.name, sub_type.name)
            }
            RpRegistered::EnumConstant { ref parent, ref variant } => {
                format!("{}.{}", parent.name, *variant.name)
            }
        }
    }
}
