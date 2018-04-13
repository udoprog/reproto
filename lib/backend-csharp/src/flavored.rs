//! C# flavor.

#![allow(unused)]

use core::errors::Result;
use core::{self, CoreFlavor, Flavor, FlavorTranslator, Loc, PackageTranslator, Translate,
           Translator};
use genco::csharp::{self, array, struct_, using};
use genco::{Cons, Csharp};
use naming::{self, Naming};
use std::collections::HashMap;
use std::ops::Deref;
use std::rc::Rc;
use trans::Packages;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CsharpFlavor;

impl Flavor for CsharpFlavor {
    type Type = Csharp<'static>;
    type Name = RpName;
    type Field = RpField;
    type Endpoint = RpEndpoint;
    type Package = core::RpPackage;
    type EnumType = Csharp<'static>;
}

/// Responsible for translating RpType -> Csharp type.
pub struct CsharpFlavorTranslator {
    packages: Rc<Packages>,
    list: Csharp<'static>,
    dictionary: Csharp<'static>,
    string: Csharp<'static>,
    date_time: Csharp<'static>,
    object: Csharp<'static>,
    pub void: Csharp<'static>,
    to_upper_camel: naming::ToUpperCamel,
}

impl CsharpFlavorTranslator {
    pub fn new(packages: Rc<Packages>) -> Self {
        Self {
            packages,
            list: using("System.Collections.Generic", "List"),
            dictionary: using("System.Collections.Generic", "Dictionary"),
            string: using("System", "String"),
            date_time: struct_(using("System", "DateTime")),
            object: using("System", "Object"),
            void: using("java.lang", "Void"),
            to_upper_camel: naming::to_upper_camel(),
        }
    }
}

impl FlavorTranslator for CsharpFlavorTranslator {
    type Source = CoreFlavor;
    type Target = CsharpFlavor;

    translator_defaults!(Self, local_name, field, endpoint);

    fn translate_i32(&self) -> Result<Csharp<'static>> {
        Ok(csharp::INT32.into())
    }

    fn translate_i64(&self) -> Result<Csharp<'static>> {
        Ok(csharp::INT64.into())
    }

    fn translate_u32(&self) -> Result<Csharp<'static>> {
        Ok(csharp::UINT32.into())
    }

    fn translate_u64(&self) -> Result<Csharp<'static>> {
        Ok(csharp::UINT64.into())
    }

    fn translate_float(&self) -> Result<Csharp<'static>> {
        Ok(csharp::SINGLE.into())
    }

    fn translate_double(&self) -> Result<Csharp<'static>> {
        Ok(csharp::DOUBLE.into())
    }

    fn translate_boolean(&self) -> Result<Csharp<'static>> {
        Ok(csharp::BOOLEAN.into())
    }

    fn translate_string(&self) -> Result<Csharp<'static>> {
        Ok(self.string.clone())
    }

    fn translate_datetime(&self) -> Result<Csharp<'static>> {
        Ok(self.date_time.clone())
    }

    fn translate_array(&self, inner: Csharp<'static>) -> Result<Csharp<'static>> {
        Ok(self.list.with_arguments(vec![inner]).into())
    }

    fn translate_map(
        &self,
        key: Csharp<'static>,
        value: Csharp<'static>,
    ) -> Result<Csharp<'static>> {
        Ok(self.dictionary.with_arguments(vec![key, value]).into())
    }

    fn translate_any(&self) -> Result<Csharp<'static>> {
        Ok(self.object.clone())
    }

    fn translate_bytes(&self) -> Result<Csharp<'static>> {
        Ok(array(csharp::BYTE))
    }

    fn translate_name(&self, reg: RpReg, name: RpName) -> Result<Csharp<'static>> {
        let package_name = Rc::new(name.package.join("."));
        let name = Rc::new(reg.ident(&name, |p| p.join("."), |c| c.join(".")));

        let ty = using(package_name, name);

        if reg.is_enum() {
            return Ok(ty.into_enum());
        } else {
            return Ok(ty);
        }
    }

    fn translate_package(&self, source: RpVersionedPackage) -> Result<RpPackage> {
        self.packages.translate_package(source)
    }

    fn translate_enum_type<T>(
        &self,
        translator: &T,
        enum_type: core::RpEnumType,
    ) -> Result<Csharp<'static>>
    where
        T: Translator<Source = Self::Source, Target = Self::Target>,
    {
        use core::RpEnumType::*;

        match enum_type {
            String => self.translate_string(),
            U32 => self.translate_u32(),
            U64 => self.translate_u64(),
            I32 => self.translate_i32(),
            I64 => self.translate_i64(),
        }
    }
}

decl_flavor!(CsharpFlavor, core);
