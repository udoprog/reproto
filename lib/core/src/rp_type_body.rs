//! Model for tuples.

use errors::Result;
use std::slice;
use translator;
use {Flavor, Loc, RpCode, Translate, Translator};

decl_body!(pub struct RpTypeBody<F> {
    pub fields: Vec<Loc<F::Field>>,
    pub codes: Vec<Loc<RpCode>>,
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

impl<F> RpTypeBody<F>
where
    F: Flavor,
{
    pub fn fields(&self) -> Fields<F> {
        Fields {
            iter: self.fields.iter(),
        }
    }
}

impl<F: 'static, T> Translate<T> for RpTypeBody<F>
where
    F: Flavor,
    T: Translator<Source = F>,
{
    type Source = F;
    type Out = RpTypeBody<T::Target>;

    /// Translate into different flavor.
    fn translate(self, translator: &T) -> Result<RpTypeBody<T::Target>> {
        translator.visit(&self.name)?;

        Ok(RpTypeBody {
            name: self.name,
            ident: self.ident,
            comment: self.comment,
            decls: self.decls.translate(translator)?,
            fields: translator::Fields(self.fields).translate(translator)?,
            codes: self.codes,
        })
    }
}
