//! # Helper trait for building a dynamic-language encode method

use base_encode::BaseEncode;
use core::errors::*;
use core::{Flavor, RpType};
use dynamic_converter::DynamicConverter;
use genco::Tokens;

pub trait DynamicEncode<'el, F>
where
    Self: DynamicConverter<'el, F>,
    F: Flavor<Type = RpType>,
{
    fn name_encode(
        &self,
        input: Tokens<'el, Self::Custom>,
        name: Tokens<'el, Self::Custom>,
    ) -> Tokens<'el, Self::Custom>;

    fn array_encode(
        &self,
        input: Tokens<'el, Self::Custom>,
        inner: Tokens<'el, Self::Custom>,
    ) -> Tokens<'el, Self::Custom>;

    fn map_encode(
        &self,
        input: Tokens<'el, Self::Custom>,
        key: Tokens<'el, Self::Custom>,
        value: Tokens<'el, Self::Custom>,
    ) -> Tokens<'el, Self::Custom>;

    /// Handle the encoding of a datetime.
    fn datetime_encode(&self, input: Tokens<'el, Self::Custom>) -> Tokens<'el, Self::Custom> {
        input
    }

    fn dynamic_encode(
        &self,
        ty: &F::Type,
        input: Tokens<'el, Self::Custom>,
    ) -> Result<Tokens<'el, Self::Custom>> {
        use core::RpType::*;

        if self.is_native(ty) {
            return Ok(input);
        }

        let stmt = match *ty {
            Signed { size: _ } | Unsigned { size: _ } => input,
            Float | Double => input,
            String => input,
            DateTime => self.datetime_encode(input),
            Any => input,
            Boolean => input,
            Name { ref name } => {
                let name = self.convert_type(name)?;
                self.name_encode(input, name)
            }
            Array { ref inner } => {
                let v = self.array_inner_var();
                let inner = self.dynamic_encode(inner, v)?;
                self.array_encode(input, inner)
            }
            Map { ref key, ref value } => {
                let map_key = self.map_key_var();
                let key = self.dynamic_encode(key, map_key)?;
                let map_value = self.map_value_var();
                let value = self.dynamic_encode(value, map_value)?;
                self.map_encode(input, key, value)
            }
            _ => input,
        };

        Ok(stmt)
    }
}

/// Dynamic encode is a valid decoding mechanism
impl<'el, T, F> BaseEncode<'el, F> for T
where
    T: DynamicEncode<'el, F>,
    F: Flavor<Type = RpType>,
{
    fn base_encode(
        &self,
        ty: &F::Type,
        input: Tokens<'el, Self::Custom>,
    ) -> Result<Tokens<'el, Self::Custom>> {
        self.dynamic_encode(ty, input)
    }
}
