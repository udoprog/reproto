//! # Helper trait for building a dynamic-language decode method

use base_decode::BaseDecode;
use converter::Converter;
use core::errors::*;
use core::{Flavor, Loc, RpInterfaceBody, RpType, WithPos};
use dynamic_converter::DynamicConverter;
use genco::Tokens;

pub trait DynamicDecode<'el, F>
where
    Self: Converter<'el, F>,
    Self: DynamicConverter<'el, F>,
    F: Flavor<Type = RpType>,
{
    fn assign_tag_var(
        &self,
        data: &'el str,
        tag_var: &'el str,
        tag: &Tokens<'el, Self::Custom>,
    ) -> Tokens<'el, Self::Custom>;

    fn check_tag_var(
        &self,
        data: &'el str,
        tag_var: &'el str,
        name: &'el str,
        type_name: Tokens<'el, Self::Custom>,
    ) -> Tokens<'el, Self::Custom>;

    fn raise_bad_type(&self, tag_var: &'el str) -> Tokens<'el, Self::Custom>;

    fn new_decode_method(
        &self,
        data: &'el str,
        body: Tokens<'el, Self::Custom>,
    ) -> Tokens<'el, Self::Custom>;

    fn name_decode(
        &self,
        input: Tokens<'el, Self::Custom>,
        name: Tokens<'el, Self::Custom>,
    ) -> Tokens<'el, Self::Custom>;

    fn array_decode(
        &self,
        input: Tokens<'el, Self::Custom>,
        inner: Tokens<'el, Self::Custom>,
    ) -> Tokens<'el, Self::Custom>;

    fn map_decode(
        &self,
        input: Tokens<'el, Self::Custom>,
        key: Tokens<'el, Self::Custom>,
        value: Tokens<'el, Self::Custom>,
    ) -> Tokens<'el, Self::Custom>;

    /// Handle the decoding of a datetime object.
    fn datetime_decode(&self, input: Tokens<'el, Self::Custom>) -> Tokens<'el, Self::Custom> {
        input
    }

    fn dynamic_decode(
        &self,
        ty: &RpType,
        input: Tokens<'el, Self::Custom>,
    ) -> Result<Tokens<'el, Self::Custom>> {
        use self::RpType::*;

        if self.is_native(ty) {
            return Ok(input);
        }

        let input = match *ty {
            Signed { size: _ } | Unsigned { size: _ } => input,
            Float | Double => input,
            String => input,
            DateTime => self.datetime_decode(input),
            Boolean => input,
            Bytes => input,
            Any => input,
            Name { ref name } => {
                let name = self.convert_type(name)?;
                self.name_decode(input, name)
            }
            Array { ref inner } => {
                let inner_var = self.array_inner_var();
                let inner = self.dynamic_decode(inner, inner_var)?;
                self.array_decode(input, inner)
            }
            Map { ref key, ref value } => {
                let map_key = self.map_key_var();
                let key = self.dynamic_decode(key, map_key)?;
                let map_value = self.map_value_var();
                let value = self.dynamic_decode(value, map_value)?;
                self.map_decode(input, key, value)
            }
        };

        Ok(input)
    }

    fn interface_decode_method(
        &self,
        body: &'el RpInterfaceBody<F>,
        tag: &Tokens<'el, Self::Custom>,
    ) -> Result<Tokens<'el, Self::Custom>> {
        let mut decode_body: Tokens<Self::Custom> = Tokens::new();

        let data = "data";
        let tag_var = "f_tag";
        decode_body.push(self.assign_tag_var(data, tag_var, tag));

        for sub_type in body.sub_types.iter() {
            let type_name = self.convert_type(&sub_type.name)
                .with_pos(Loc::pos(sub_type))?;
            decode_body.push(self.check_tag_var(data, tag_var, sub_type.name(), type_name));
        }

        decode_body.push(self.raise_bad_type(tag_var));

        Ok(self.new_decode_method(data, decode_body.join_line_spacing()))
    }
}

/// Dynamic decode is a valid decoding mechanism
impl<'el, T, F> BaseDecode<'el, F> for T
where
    T: DynamicDecode<'el, F>,
    F: Flavor<Type = RpType>,
{
    fn base_decode(
        &self,
        ty: &RpType,
        input: Tokens<'el, Self::Custom>,
    ) -> Result<Tokens<'el, Self::Custom>> {
        self.dynamic_decode(ty, input)
    }
}
