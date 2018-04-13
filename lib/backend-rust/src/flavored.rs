//! Rust flavor.

#![allow(unused)]

use core::errors::Result;
use core::{self, CoreFlavor, Flavor, FlavorTranslator, Loc, PackageTranslator, Translate,
           Translator};
use genco::rust;
use genco::{Cons, Rust};
use std::collections::HashMap;
use std::ops::Deref;
use std::rc::Rc;
use trans::Packages;
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RustFlavor;

impl Flavor for RustFlavor {
    type Type = Rust<'static>;
    type Name = RpName;
    type Field = core::RpField<RustFlavor>;
    type Endpoint = RustEndpoint;
    type Package = core::RpPackage;
    type EnumType = Rust<'static>;
}

/// Responsible for translating RpType -> Rust type.
pub struct RustFlavorTranslator {
    packages: Rc<Packages>,
    map: Rust<'static>,
    json_value: Rust<'static>,
    datetime: Option<Rust<'static>>,
}

impl RustFlavorTranslator {
    pub fn new(packages: Rc<Packages>, datetime: Option<Rust<'static>>) -> Self {
        Self {
            packages,
            map: rust::imported("std::collections", "HashMap"),
            json_value: rust::imported("serde_json", "Value").alias("json"),
            datetime: datetime,
        }
    }
}

impl FlavorTranslator for RustFlavorTranslator {
    type Source = CoreFlavor;
    type Target = RustFlavor;

    translator_defaults!(Self, local_name, field);

    fn translate_i32(&self) -> Result<Rust<'static>> {
        Ok(rust::local("i32"))
    }

    fn translate_i64(&self) -> Result<Rust<'static>> {
        Ok(rust::local("i64"))
    }

    fn translate_u32(&self) -> Result<Rust<'static>> {
        Ok(rust::local("u32"))
    }

    fn translate_u64(&self) -> Result<Rust<'static>> {
        Ok(rust::local("u64"))
    }

    fn translate_float(&self) -> Result<Rust<'static>> {
        Ok(rust::local("f32"))
    }

    fn translate_double(&self) -> Result<Rust<'static>> {
        Ok(rust::local("f64"))
    }

    fn translate_boolean(&self) -> Result<Rust<'static>> {
        Ok(rust::local("bool"))
    }

    fn translate_string(&self) -> Result<Rust<'static>> {
        Ok(rust::local("String"))
    }

    fn translate_datetime(&self) -> Result<Rust<'static>> {
        if let Some(ref datetime) = self.datetime {
            return Ok(datetime.clone());
        }

        Err("Missing implementation for `datetime`, try: -m chrono".into())
    }

    fn translate_array(&self, argument: Rust<'static>) -> Result<Rust<'static>> {
        Ok(rust::local("Vec").with_arguments(vec![argument]))
    }

    fn translate_map(&self, key: Rust<'static>, value: Rust<'static>) -> Result<Rust<'static>> {
        Ok(self.map.clone().with_arguments(vec![key, value]))
    }

    fn translate_any(&self) -> Result<Rust<'static>> {
        Ok(self.json_value.clone())
    }

    fn translate_bytes(&self) -> Result<Rust<'static>> {
        Ok(rust::local("String"))
    }

    fn translate_name(&self, reg: RpReg, name: RpName) -> Result<Rust<'static>> {
        let ident = reg.ident(&name, |p| p.join(TYPE_SEP), |c| c.join(SCOPE_SEP));

        if let Some(prefix) = name.prefix {
            let package_name = name.package.join("::");
            let (prefix, _) = Loc::take_pair(prefix);
            return Ok(rust::imported(package_name, ident).alias(prefix));
        }

        Ok(rust::local(ident))
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

    fn translate_package(&self, source: RpVersionedPackage) -> Result<RpPackage> {
        self.packages.translate_package(source)
    }

    fn translate_enum_type<T>(
        &self,
        translator: &T,
        enum_type: core::RpEnumType,
    ) -> Result<Rust<'static>>
    where
        T: Translator<Source = Self::Source, Target = Self::Target>,
    {
        use core::RpEnumType::*;

        match enum_type {
            String => Ok(rust::local("str").reference(rust::StaticRef)),
            U32 => self.translate_u32(),
            U64 => self.translate_u64(),
            I32 => self.translate_i32(),
            I64 => self.translate_i64(),
        }
    }
}

decl_flavor!(RustFlavor, core);
