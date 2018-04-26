//! Trait used to handle imports.

use errors::Result;
use {RpRequiredPackage, RpVersionedPackage};

pub trait Import {
    /// Perform the import.
    fn import(&mut self, &RpRequiredPackage) -> Result<Option<RpVersionedPackage>>;
}
