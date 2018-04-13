use core::RpPackage;
use core::errors::*;

pub trait IntoBytes<Processor> {
    fn into_bytes(self, processor: &Processor, package: &RpPackage) -> Result<Vec<u8>>;
}
