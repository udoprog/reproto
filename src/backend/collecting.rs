use super::errors::*;

pub trait Collecting {
    type Processor;

    fn new() -> Self;

    fn into_bytes(self, processor: &Self::Processor) -> Result<Vec<u8>>;
}
