//! Model for tuples.

use errors::Result;
use std::slice;
use translator;
use {Flavor, Loc, RpCode, RpSubType, Translate, Translator};

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
        RpSubTypeStrategy::Tagged {
            tag: DEFAULT_TAG.to_string(),
        }
    }
}

decl_body!(pub struct RpInterfaceBody<F> {
    pub fields: Vec<Loc<F::Field>>,
    pub codes: Vec<Loc<RpCode>>,
    pub sub_types: Vec<Loc<RpSubType<F>>>,
    pub sub_type_strategy: RpSubTypeStrategy,
});

/// Iterator over fields.
pub struct Fields<'a, F: 'static>
where
    F: Flavor,
{
    iter: slice::Iter<'a, Loc<F::Field>>,
}

impl<'a, F: 'static> Iterator for Fields<'a, F>
where
    F: Flavor,
{
    type Item = &'a Loc<F::Field>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl<F: 'static> RpInterfaceBody<F>
where
    F: Flavor,
{
    pub fn fields(&self) -> Fields<F> {
        Fields {
            iter: self.fields.iter(),
        }
    }
}

impl<F: 'static, T> Translate<T> for RpInterfaceBody<F>
where
    F: Flavor,
    T: Translator<Source = F>,
{
    type Source = F;
    type Out = RpInterfaceBody<T::Target>;

    /// Translate into different flavor.
    fn translate(self, translator: &T) -> Result<RpInterfaceBody<T::Target>> {
        translator.visit(&self.name)?;

        Ok(RpInterfaceBody {
            name: self.name,
            ident: self.ident,
            comment: self.comment,
            decls: self.decls.translate(translator)?,
            fields: translator::Fields(self.fields).translate(translator)?,
            codes: self.codes,
            sub_types: self.sub_types.translate(translator)?,
            sub_type_strategy: self.sub_type_strategy,
        })
    }
}
