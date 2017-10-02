use super::{Loc, RpCode, RpDecl, RpEnumBody, RpField, RpInterfaceBody, RpServiceBody, RpSubType,
            RpTupleBody, RpTypeBody};
use super::errors::*;
use std::collections::BTreeMap;
use std::collections::btree_map;
use std::rc::Rc;

/// Merging of models.
pub trait Merge {
    /// Merge the current model with another.
    fn merge(&mut self, other: Self) -> Result<()>;
}

impl<T> Merge for Rc<T>
where
    T: Merge,
{
    fn merge(&mut self, source: Rc<T>) -> Result<()> {
        let rc = Rc::get_mut(self).ok_or(ErrorKind::RcGetMut)?;
        let source = Rc::try_unwrap(source).map_err(|_| ErrorKind::RcTryUnwrap)?;
        rc.merge(source)?;
        Ok(())
    }
}

impl<K, T> Merge for BTreeMap<K, T>
where
    T: Merge,
    K: ::std::cmp::Ord,
{
    fn merge(&mut self, source: BTreeMap<K, T>) -> Result<()> {
        for (key, value) in source {
            match self.entry(key) {
                btree_map::Entry::Vacant(entry) => {
                    entry.insert(value);
                }
                btree_map::Entry::Occupied(entry) => {
                    Merge::merge(entry.into_mut(), value)?;
                }
            }
        }

        Ok(())
    }
}

impl Merge for Vec<Loc<RpCode>> {
    fn merge(&mut self, source: Vec<Loc<RpCode>>) -> Result<()> {
        self.extend(source);
        Ok(())
    }
}

impl Merge for Loc<RpDecl> {
    fn merge(&mut self, source: Loc<RpDecl>) -> Result<()> {
        use self::RpDecl::*;

        let dest_pos = self.pos().clone();
        let m = self.as_mut();

        match *m {
            Type(ref mut body) => {
                if let Type(ref other) = *source {
                    return body.merge(other.clone());
                }
            }
            Enum(ref mut body) => {
                if let Enum(ref other) = *source {
                    if let Some(variant) = other.variants.iter().next() {
                        return Err(
                            ErrorKind::ExtendEnum(
                                "cannot extend enum with additional variants".to_owned(),
                                variant.pos().into(),
                                dest_pos.into(),
                            ).into(),
                        );
                    }

                    return body.merge(other.clone());
                }
            }
            Interface(ref mut body) => {
                if let Interface(ref other) = *source {
                    return body.merge(other.clone());
                }
            }
            Tuple(ref mut body) => {
                if let Tuple(ref other) = *source {
                    return body.merge(other.clone());
                }
            }
            Service(ref mut body) => {
                if let Service(ref other) = *source {
                    return body.merge(other.clone());
                }
            }
        }

        return Err(
            ErrorKind::DeclMerge(
                format!("cannot merge with {}", source),
                source.pos().into(),
                dest_pos.into(),
            ).into(),
        );
    }
}

impl Merge for RpEnumBody {
    fn merge(&mut self, source: RpEnumBody) -> Result<()> {
        self.codes.merge(source.codes)?;
        Ok(())
    }
}

impl Merge for Vec<Loc<RpField>> {
    fn merge(&mut self, source: Vec<Loc<RpField>>) -> Result<()> {
        for f in source {
            if let Some(field) = self.iter().find(|e| e.name == f.name) {
                return Err(
                    ErrorKind::FieldConflict(f.name.clone(), f.pos().into(), field.pos().into())
                        .into(),
                );
            }

            self.push(f);
        }

        Ok(())
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

impl Merge for RpServiceBody {
    fn merge(&mut self, _source: RpServiceBody) -> Result<()> {
        Ok(())
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

impl Merge for RpTupleBody {
    fn merge(&mut self, source: RpTupleBody) -> Result<()> {
        self.fields.merge(source.fields)?;
        self.codes.merge(source.codes)?;
        Ok(())
    }
}

impl Merge for RpTypeBody {
    fn merge(&mut self, source: RpTypeBody) -> Result<()> {
        self.fields.merge(source.fields)?;
        self.codes.merge(source.codes)?;
        Ok(())
    }
}
