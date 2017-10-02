//! # Helper trait to deal with value construction

use converter::Converter;
use core::{RpEnumOrdinal, RpEnumVariant};
use errors::*;

pub trait ValueBuilder
where
    Self: Converter,
{
    /// Convert the string to a statement.
    fn string(&self, &str) -> Result<Self::Stmt>;

    fn ordinal(&self, variant: &RpEnumVariant) -> Result<Self::Stmt> {
        use self::RpEnumOrdinal::*;

        match variant.ordinal {
            String(ref string) => self.string(string),
            Generated => self.string(&variant.local_name),
        }
    }
}
