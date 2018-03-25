//! Variant in an enum

use errors::Result;
use {Flavor, Loc, RpEnumOrdinal, RpName, Translate, Translator};

#[derive(Debug, Clone, Serialize)]
pub struct RpVariant {
    pub name: RpName,
    pub ident: Loc<String>,
    pub comment: Vec<String>,
    pub ordinal: RpEnumOrdinal,
}

impl RpVariant {
    /// Get the identifier of the variant.
    pub fn ident(&self) -> &str {
        self.ident.as_str()
    }

    /// Get the ordinal value of the variant.
    pub fn ordinal(&self) -> &str {
        use self::RpEnumOrdinal::*;

        match self.ordinal {
            String(ref string) => string.as_str(),
            Generated => self.ident(),
        }
    }
}

impl<F: 'static, T> Translate<T> for RpVariant
where
    F: Flavor,
    T: Translator<Source = F>,
{
    type Source = F;
    type Out = RpVariant;

    /// Translate into different flavor.
    fn translate(self, translator: &T) -> Result<RpVariant> {
        translator.visit(&self.name)?;

        Ok(RpVariant {
            name: self.name,
            ident: self.ident,
            comment: self.comment,
            ordinal: self.ordinal,
        })
    }
}
