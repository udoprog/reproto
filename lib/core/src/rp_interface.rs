//! Model for tuples.

use errors::Result;
use serde::Serialize;
use std::slice;
use std::vec;
use translator;
use {Flavor, FlavorField, Loc, RpCode, RpDecl, RpReg, Translate, Translator};

/// Default key to use for tagged sub type strategy.
pub const DEFAULT_TAG: &str = "type";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RpSubTypeStrategy {
    /// An object, with a single tag key indicating which sub-type to use.
    Tagged { tag: String },
    /// An sub-type is distinguished by its set of unique fields.
    /// This requires a sub-type to actually _have_ a unique set of fields, which is validates
    /// during translation.
    Untagged,
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

/// Iterator over discriminating sub-type fields.
///
/// Discriminating fields are ones which makes this sub-type unique, and are used to distinguish
/// between different untagged sub-types.
pub struct DiscriminatingFields<'a, F: 'static>
where
    F: Flavor,
{
    iter: vec::IntoIter<&'a Loc<F::Field>>,
}

impl<'a, F: 'static> Iterator for DiscriminatingFields<'a, F>
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

        let name = translator.translate_local_name(RpReg::Interface, self.name)?;

        Ok(RpInterfaceBody {
            name: name,
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

#[derive(Debug, Clone, Serialize)]
#[serde(bound = "F: Serialize, F::Field: Serialize, F::Endpoint: Serialize, F::Package: \
                 Serialize, F::Name: Serialize, F::EnumType: Serialize")]
pub struct RpSubType<F: 'static>
where
    F: Flavor,
{
    pub name: F::Name,
    pub ident: String,
    pub comment: Vec<String>,
    /// Inner declarations.
    pub decls: Vec<RpDecl<F>>,
    pub fields: Vec<Loc<F::Field>>,
    pub codes: Vec<Loc<RpCode>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_type_name: Option<Loc<String>>,
}

impl<F: 'static> RpSubType<F>
where
    F: Flavor,
{
    pub fn name(&self) -> &str {
        self.sub_type_name
            .as_ref()
            .map(|t| t.as_str())
            .unwrap_or(&self.ident)
    }

    /// Access the set of fields which are used to make this sub-type unique.
    pub fn discriminating_fields(&self) -> DiscriminatingFields<F> {
        let fields = self.fields
            .iter()
            .filter(|f| f.is_discriminating())
            .collect::<Vec<_>>();

        DiscriminatingFields {
            iter: fields.into_iter(),
        }
    }
}

impl<F: 'static, T> Translate<T> for RpSubType<F>
where
    F: Flavor,
    T: Translator<Source = F>,
{
    type Source = F;
    type Out = RpSubType<T::Target>;

    /// Translate into different flavor.
    fn translate(self, translator: &T) -> Result<RpSubType<T::Target>> {
        translator.visit(&self.name)?;

        let name = translator.translate_local_name(RpReg::SubType, self.name)?;

        Ok(RpSubType {
            name: name,
            ident: self.ident,
            comment: self.comment,
            decls: self.decls.translate(translator)?,
            fields: translator::Fields(self.fields).translate(translator)?,
            codes: self.codes,
            sub_type_name: self.sub_type_name,
        })
    }
}
