//! # Helper trait for building a dynamic-language encode method

use super::converter::Converter;
use core::errors::*;
use core::{Flavor, RpType};
use genco::Tokens;

pub trait BaseEncode<'el, F>
where
    Self: Converter<'el, F>,
    F: Flavor<Type = RpType>,
{
    fn base_encode(
        &self,
        ty: &'el RpType,
        input: Tokens<'el, Self::Custom>,
    ) -> Result<Tokens<'el, Self::Custom>>;
}
