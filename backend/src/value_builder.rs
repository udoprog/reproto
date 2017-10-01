//! # Helper trait to deal with value construction
//!
//! RpValue construction is when a literal value is encoded into the output.
//!
//! For example, when creating an instance of type `Foo(1, 2, 3)` in java could be translated to:
//!
//! ```java
//! new Foo(1, 2F, 3D)
//! ```
//!
//! In this example, the second field is a `float`, and the third field is a `double`.

use converter::Converter;
use core::RpEnumOrdinal;
use errors::*;

pub trait ValueBuilder
where
    Self: Converter,
{
    fn string(&self, &str) -> Result<Self::Stmt>;

    fn ordinal_number(&self, &u32) -> Result<Self::Stmt>;

    fn ordinal(&self, ordinal: &RpEnumOrdinal) -> Result<Self::Stmt> {
        use self::RpEnumOrdinal::*;

        match *ordinal {
            String(ref string) => self.string(string),
            Number(ref number) => self.ordinal_number(number),
        }
    }
}
