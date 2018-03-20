//! Model for tuples.

use super::{Loc, RpCode, RpField, RpSubType};
use std::rc::Rc;
use std::slice;

/// Default key to use for tagged sub type strategy.
pub const DEFAULT_TAG: &str = "type";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RpSubTypeStrategy {
    /// An object, with a single tag key indicating which sub-type to use.
    Tagged { tag: String },
}

impl Default for RpSubTypeStrategy {
    fn default() -> Self {
        RpSubTypeStrategy::Tagged { tag: DEFAULT_TAG.to_string() }
    }
}

decl_body!(pub struct RpInterfaceBody {
    pub fields: Vec<Loc<RpField>>,
    pub codes: Vec<Loc<RpCode>>,
    pub sub_types: Vec<Rc<Loc<RpSubType>>>,
    pub sub_type_strategy: RpSubTypeStrategy,
});

/// Iterator over fields.
pub struct Fields<'a> {
    iter: slice::Iter<'a, Loc<RpField>>,
}

impl<'a> Iterator for Fields<'a> {
    type Item = &'a Loc<RpField>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl RpInterfaceBody {
    pub fn fields(&self) -> Fields {
        Fields { iter: self.fields.iter() }
    }
}
