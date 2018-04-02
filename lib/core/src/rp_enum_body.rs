//! Model for enums

use errors::Result;
use {Flavor, Loc, RpCode, RpEnumType, RpVariant, Translate, Translator};

decl_body!(pub struct RpEnumBody<F> {
    /// The type of the variant.
    pub enum_type: RpEnumType,
    /// Variants in the enum.
    pub variants: Vec<Loc<RpVariant<F>>>,
    /// Custom code blocks in the enum.
    pub codes: Vec<Loc<RpCode>>,
});

impl<F: 'static> RpEnumBody<F>
where
    F: Flavor,
{
    /// Translate into different flavor.
    pub fn translate<T>(self, translator: &T) -> Result<RpEnumBody<T::Target>>
    where
        T: Translator<Source = F>,
    {
        Ok(RpEnumBody {
            name: self.name.translate(translator)?,
            ident: self.ident,
            comment: self.comment,
            decls: self.decls.translate(translator)?,
            enum_type: self.enum_type,
            variants: self.variants.translate(translator)?,
            codes: self.codes,
        })
    }
}

impl<F: 'static, T> Translate<T> for RpEnumBody<F>
where
    F: Flavor,
    T: Translator<Source = F>,
{
    type Source = F;
    type Out = RpEnumBody<T::Target>;

    /// Translate into different flavor.
    fn translate(self, translator: &T) -> Result<RpEnumBody<T::Target>> {
        translator.visit(&self.name)?;

        let name = self.name.translate(translator)?;

        Ok(RpEnumBody {
            name: name,
            ident: self.ident,
            comment: self.comment,
            decls: self.decls.translate(translator)?,
            enum_type: self.enum_type,
            variants: self.variants.translate(translator)?,
            codes: self.codes,
        })
    }
}
