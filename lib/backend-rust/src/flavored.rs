//! Rust flavor.

#![allow(unused)]

use backend::PackageUtils;
use core::errors::Result;
use core::{self, CoreFlavor, Flavor, Loc, Translate, Translator, TypeTranslator};
use genco::rust::{imported, local};
use genco::{Cons, Rust};
use std::ops::Deref;
use std::rc::Rc;
use {SCOPE_SEP, TYPE_SEP};

#[derive(Debug, Clone)]
pub struct RustEndpoint {
    pub endpoint: RpEndpoint,
    pub http1: Option<RpEndpointHttp1>,
}

impl Deref for RustEndpoint {
    type Target = RpEndpoint;

    fn deref(&self) -> &Self::Target {
        &self.endpoint
    }
}

#[derive(Debug, Clone)]
pub struct RustFlavor;

impl Flavor for RustFlavor {
    type Type = Rust<'static>;
    type Field = core::RpField<RustFlavor>;
    type Endpoint = RustEndpoint;
}

/// Responsible for translating RpType -> Rust type.
pub struct RustTypeTranslator {
    map: Rust<'static>,
    json_value: Rust<'static>,
    datetime: Option<Rust<'static>>,
}

impl RustTypeTranslator {
    pub fn new(datetime: Option<Rust<'static>>) -> Self {
        Self {
            map: imported("std::collections", "HashMap"),
            json_value: imported("serde_json", "Value").alias("json"),
            datetime: datetime,
        }
    }
}

impl PackageUtils for RustTypeTranslator {}

impl TypeTranslator for RustTypeTranslator {
    type Source = CoreFlavor;
    type Target = RustFlavor;

    fn translate_i32(&self) -> Result<Rust<'static>> {
        Ok(local("i32"))
    }

    fn translate_i64(&self) -> Result<Rust<'static>> {
        Ok(local("i64"))
    }

    fn translate_u32(&self) -> Result<Rust<'static>> {
        Ok(local("u32"))
    }

    fn translate_u64(&self) -> Result<Rust<'static>> {
        Ok(local("u64"))
    }

    fn translate_float(&self) -> Result<Rust<'static>> {
        Ok(local("f32"))
    }

    fn translate_double(&self) -> Result<Rust<'static>> {
        Ok(local("f64"))
    }

    fn translate_boolean(&self) -> Result<Rust<'static>> {
        Ok(local("bool"))
    }

    fn translate_string(&self) -> Result<Rust<'static>> {
        Ok(local("String"))
    }

    fn translate_datetime(&self) -> Result<Rust<'static>> {
        if let Some(ref datetime) = self.datetime {
            return Ok(datetime.clone());
        }

        Err("Missing implementation for `datetime`, try: -m chrono".into())
    }

    fn translate_array(&self, argument: Rust<'static>) -> Result<Rust<'static>> {
        Ok(local("Vec").with_arguments(vec![argument]))
    }

    fn translate_map(&self, key: Rust<'static>, value: Rust<'static>) -> Result<Rust<'static>> {
        Ok(self.map.with_arguments(vec![key, value]))
    }

    fn translate_any(&self) -> Result<Rust<'static>> {
        Ok(self.json_value.clone())
    }

    fn translate_bytes(&self) -> Result<Rust<'static>> {
        Ok(local("String"))
    }

    fn translate_name(&self, name: RpName, reg: RpReg) -> Result<Rust<'static>> {
        let ident = reg.ident(&name, |p| p.join(TYPE_SEP), |c| c.join(SCOPE_SEP));

        if let Some(prefix) = name.prefix {
            let package_name = self.package(&name.package).parts.join("::");
            return Ok(imported(package_name, ident).alias(prefix));
        }

        Ok(local(ident))
    }

    fn translate_field<T>(
        &self,
        translator: &T,
        field: core::RpField<CoreFlavor>,
    ) -> Result<core::RpField<RustFlavor>>
    where
        T: Translator<Source = CoreFlavor, Target = RustFlavor>,
    {
        field.translate(translator)
    }

    fn translate_endpoint<T>(
        &self,
        translator: &T,
        endpoint: core::RpEndpoint<CoreFlavor>,
    ) -> Result<RustEndpoint>
    where
        T: Translator<Source = CoreFlavor, Target = RustFlavor>,
    {
        let endpoint = endpoint.translate(translator)?;
        let http1 = RpEndpointHttp1::from_endpoint(&endpoint);

        Ok(RustEndpoint { endpoint, http1 })
    }
}

decl_flavor!(RustFlavor, core);
