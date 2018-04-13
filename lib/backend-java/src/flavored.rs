//! Java flavor.

#![allow(unused)]

use backend::package_processor;
use core::errors::Result;
use core::{self, CoreFlavor, Flavor, FlavorField, FlavorTranslator, Loc, PackageTranslator,
           Translate, Translator};
use genco::java::{self, Argument, Field, Method, Modifier, BOOLEAN, DOUBLE, FLOAT, INTEGER, LONG,
                  VOID};
use genco::{Cons, Element, Java};
use naming::{self, Naming};
use std::collections::HashMap;
use std::fmt;
use std::ops::Deref;
use std::rc::Rc;
use trans::Packages;

#[derive(Debug, Clone)]
pub struct JavaHttp<'el> {
    pub request: Java<'el>,
    pub response: Java<'el>,
    pub path: RpPathSpec,
    pub method: RpHttpMethod,
}

#[derive(Debug, Clone)]
pub struct JavaEndpoint<'el> {
    pub endpoint: RpEndpoint,
    pub arguments: Vec<Argument<'el>>,
    pub http1: Option<RpEndpointHttp1>,
}

impl<'el> Deref for JavaEndpoint<'el> {
    type Target = RpEndpoint;

    fn deref(&self) -> &Self::Target {
        &self.endpoint
    }
}

/// A single field.
#[derive(Debug, Clone)]
pub struct JavaField<'el> {
    pub field: RpField,
    pub field_accessor: Rc<String>,
    pub spec: Field<'el>,
}

impl<'el> FlavorField for JavaField<'el> {
    fn is_discriminating(&self) -> bool {
        self.field.is_discriminating()
    }
}

impl<'el> ::std::ops::Deref for JavaField<'el> {
    type Target = RpField;

    fn deref(&self) -> &Self::Target {
        &self.field
    }
}

impl<'el> JavaField<'el> {
    pub fn setter(&self) -> Option<Method<'el>> {
        if self.spec.modifiers.contains(&Modifier::Final) {
            return None;
        }

        let argument = Argument::new(self.spec.ty(), self.spec.var());
        let mut m = Method::new(Rc::new(format!("set{}", self.field_accessor)));

        m.arguments.push(argument.clone());

        m.body
            .push(toks!["this.", self.spec.var(), " = ", argument.var(), ";",]);

        Some(m)
    }

    /// Create a new getter method without a body.
    pub fn getter_without_body(&self) -> Method<'el> {
        // Avoid `getClass`, a common built-in method for any Object.
        let field_accessor = match self.field_accessor.as_str() {
            "Class" => "Class_",
            accessor => accessor,
        };

        let mut method = Method::new(Rc::new(format!("get{}", field_accessor)));
        method.comments = self.spec.comments.clone();
        method.returns = self.spec.ty().as_field();
        method
    }

    /// Build a new complete getter.
    pub fn getter(&self) -> Method<'el> {
        let mut m = self.getter_without_body();
        m.body.push(toks!["return this.", self.spec.var(), ";"]);
        m
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaName {
    pub name: Rc<String>,
    pub package: RpPackage,
}

impl fmt::Display for JavaName {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        self.name.fmt(fmt)
    }
}

impl<'el> From<&'el JavaName> for Element<'el, Java<'el>> {
    fn from(value: &'el JavaName) -> Element<'el, Java<'el>> {
        Element::Literal(value.name.to_string().into())
    }
}

impl package_processor::Name<JavaFlavor> for JavaName {
    fn package(&self) -> &RpPackage {
        &self.package
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct JavaFlavor;

impl Flavor for JavaFlavor {
    type Type = Java<'static>;
    type Name = JavaName;
    type Field = JavaField<'static>;
    type Endpoint = JavaEndpoint<'static>;
    type Package = core::RpPackage;
    type EnumType = Java<'static>;
}

/// Responsible for translating RpType -> Java type.
pub struct JavaFlavorTranslator {
    packages: Rc<Packages>,
    list: Java<'static>,
    map: Java<'static>,
    string: Java<'static>,
    instant: Java<'static>,
    object: Java<'static>,
    byte_buffer: Java<'static>,
    optional: Java<'static>,
    to_upper_camel: naming::ToUpperCamel,
    to_lower_camel: naming::ToLowerCamel,
}

impl JavaFlavorTranslator {
    pub fn new(packages: Rc<Packages>) -> Self {
        Self {
            packages,
            list: java::imported("java.util", "List"),
            map: java::imported("java.util", "Map"),
            string: java::imported("java.lang", "String"),
            instant: java::imported("java.time", "Instant"),
            object: java::imported("java.lang", "Object"),
            byte_buffer: java::imported("java.nio", "ByteBuffer"),
            optional: java::imported("java.util", "Optional"),
            to_upper_camel: naming::to_upper_camel(),
            to_lower_camel: naming::to_lower_camel(),
        }
    }
}

impl FlavorTranslator for JavaFlavorTranslator {
    type Source = CoreFlavor;
    type Target = JavaFlavor;

    translator_defaults!(Self);

    fn translate_i32(&self) -> Result<Java<'static>> {
        Ok(INTEGER.into())
    }

    fn translate_i64(&self) -> Result<Java<'static>> {
        Ok(LONG.into())
    }

    fn translate_u32(&self) -> Result<Java<'static>> {
        Ok(INTEGER.into())
    }

    fn translate_u64(&self) -> Result<Java<'static>> {
        Ok(LONG.into())
    }

    fn translate_float(&self) -> Result<Java<'static>> {
        Ok(FLOAT.into())
    }

    fn translate_double(&self) -> Result<Java<'static>> {
        Ok(DOUBLE.into())
    }

    fn translate_boolean(&self) -> Result<Java<'static>> {
        Ok(BOOLEAN.into())
    }

    fn translate_string(&self) -> Result<Java<'static>> {
        Ok(self.string.clone().into())
    }

    fn translate_datetime(&self) -> Result<Java<'static>> {
        Ok(self.instant.clone().into())
    }

    fn translate_array(&self, argument: Java<'static>) -> Result<Java<'static>> {
        Ok(self.list.with_arguments(vec![argument]))
    }

    fn translate_map(&self, key: Java<'static>, value: Java<'static>) -> Result<Java<'static>> {
        Ok(self.map.with_arguments(vec![key, value]))
    }

    fn translate_any(&self) -> Result<Java<'static>> {
        Ok(self.object.clone())
    }

    fn translate_bytes(&self) -> Result<Java<'static>> {
        Ok(self.byte_buffer.clone())
    }

    fn translate_name(&self, reg: RpReg, name: RpName) -> Result<Java<'static>> {
        let ident = Rc::new(reg.ident(&name, |p| p.join("."), |c| c.join(".")));
        let package = name.package.join(".");
        Ok(java::imported(package, ident))
    }

    fn translate_field<T>(
        &self,
        translator: &T,
        field: core::RpField<CoreFlavor>,
    ) -> Result<JavaField<'static>>
    where
        T: Translator<Source = CoreFlavor, Target = JavaFlavor>,
    {
        let mut field = field.translate(translator)?;

        let field_accessor = Rc::new(self.to_upper_camel.convert(field.ident()));

        let java_type = if field.is_optional() {
            java::optional(
                field.ty.clone(),
                self.optional.with_arguments(vec![field.ty.clone()]),
            )
        } else {
            field.ty.clone()
        };

        let mut spec = Field::new(java_type, field.safe_ident().to_string());

        if !field.comment.is_empty() {
            spec.comments.push("<pre>".into());
            spec.comments
                .extend(field.comment.drain(..).map(Cons::from));
            spec.comments.push("</pre>".into());
        }

        Ok(JavaField {
            field,
            field_accessor: field_accessor,
            spec: spec,
        })
    }

    fn translate_endpoint<T>(
        &self,
        translator: &T,
        endpoint: core::RpEndpoint<CoreFlavor>,
    ) -> Result<JavaEndpoint<'static>>
    where
        T: Translator<Source = CoreFlavor, Target = JavaFlavor>,
    {
        let mut endpoint = endpoint.translate(translator)?;

        let mut arguments = Vec::new();

        for arg in &endpoint.arguments {
            let ty = arg.channel.ty().clone();
            arguments.push(Argument::new(ty, arg.safe_ident().to_string()));
        }

        let http1 = RpEndpointHttp1::from_endpoint(&endpoint);

        // Used by Closeable implementations.
        match endpoint.safe_ident() {
            "close" => {
                endpoint.safe_ident = Some("close_".to_string());
            }
            _ => {}
        }

        return Ok(JavaEndpoint {
            endpoint: endpoint,
            arguments: arguments,
            http1: http1,
        });
    }

    fn translate_package(&self, source: RpVersionedPackage) -> Result<RpPackage> {
        self.packages.translate_package(source)
    }

    fn translate_local_name<T>(
        &self,
        translator: &T,
        reg: RpReg,
        name: core::RpName<CoreFlavor>,
    ) -> Result<JavaName>
    where
        T: Translator<Source = Self::Source, Target = Self::Target>,
    {
        let ident = Rc::new(reg.ident(&name, |p| p.join("."), |c| c.join(".")));
        let package = self.translate_package(name.package)?;

        Ok(JavaName {
            name: ident,
            package,
        })
    }

    fn translate_enum_type<T>(
        &self,
        translator: &T,
        enum_type: core::RpEnumType,
    ) -> Result<Java<'static>>
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

decl_flavor!(JavaFlavor, core);
