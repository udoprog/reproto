//! Java flavor.

#![allow(unused)]

use core::{self, CoreFlavor, Flavor, Loc, Translate, Translator, TypeTranslator};
use core::errors::Result;
use genco::{Cons, Java};
use genco::java::{imported, optional, Argument, Field, Method, Modifier, BOOLEAN, DOUBLE, FLOAT,
                  INTEGER, LONG, VOID};
use naming::{self, Naming};
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct JavaEndpoint<'el> {
    pub ident: Cons<'el>,
    pub name: Cons<'el>,
    pub response: Option<Loc<RpChannel>>,
    pub response_ty: Java<'el>,
    pub request: Option<RpEndpointArgument>,
    pub request_ty: Java<'el>,
    pub arguments: Vec<Argument<'el>>,
    pub comment: Vec<String>,
    pub http: RpEndpointHttp,
}

no_serializer!(JavaEndpoint<'a>);

impl<'el> JavaEndpoint<'el> {
    /// If endpoint has metadata for HTTP.
    pub fn has_http_support(&self) -> bool {
        if !self.http.path.is_some() {
            return false;
        }

        true
    }
}

/// A single field.
#[derive(Debug, Clone)]
pub struct JavaField<'el> {
    pub name: Cons<'el>,
    pub ident: Rc<String>,
    pub field_accessor: Rc<String>,
    pub spec: Field<'el>,
}

no_serializer!(JavaField<'a>);

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

    /// The JSON name of the field.
    pub fn name(&self) -> Cons<'el> {
        self.name.clone()
    }
}

#[derive(Debug, Clone)]
pub struct JavaFlavor;

impl Flavor for JavaFlavor {
    type Type = Java<'static>;
    type Field = JavaField<'static>;
    type Endpoint = JavaEndpoint<'static>;
}

/// Responsible for translating RpType -> Java type.
pub struct JavaTypeTranslator {
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

impl JavaTypeTranslator {
    pub fn new() -> Self {
        Self {
            list: imported("java.util", "List"),
            map: imported("java.util", "Map"),
            string: imported("java.lang", "String"),
            instant: imported("java.time", "Instant"),
            object: imported("java.lang", "Object"),
            byte_buffer: imported("java.nio", "ByteBuffer"),
            optional: imported("java.util", "Optional"),
            to_upper_camel: naming::to_upper_camel(),
            to_lower_camel: naming::to_lower_camel(),
        }
    }
}

impl TypeTranslator for JavaTypeTranslator {
    type Source = CoreFlavor;
    type Target = JavaFlavor;

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

    fn translate_name(&self, name: RpName, reg: RpReg) -> Result<Java<'static>> {
        let pkg = name.package
            .as_package(|version| format!("_{}", version).replace(".", "_").replace("-", "_"));
        let package_name = Rc::new(pkg.parts.join("."));
        let name = Rc::new(reg.ident(&name, |p| p.join("."), |c| c.join(".")));
        Ok(imported(package_name, name))
    }

    fn translate_field<T>(
        &self,
        translator: &T,
        field: core::RpField<CoreFlavor>,
    ) -> Result<JavaField<'static>>
    where
        T: Translator<Source = CoreFlavor, Target = JavaFlavor>,
    {
        let ty = translator.translate_type(field.ty.clone())?;

        let java_type = if field.is_optional() {
            optional(ty.clone(), self.optional.with_arguments(vec![ty.clone()]))
        } else {
            ty
        };

        let ident = Rc::new(self.to_lower_camel.convert(field.safe_ident()));
        let field_accessor = Rc::new(self.to_upper_camel.convert(field.ident()));

        let name = Cons::from(field.name().to_string());

        let mut spec = Field::new(java_type, ident.clone());

        if !field.comment.is_empty() {
            spec.comments.push("<pre>".into());
            spec.comments
                .extend(field.comment.into_iter().map(Cons::from));
            spec.comments.push("</pre>".into());
        }

        Ok(JavaField {
            name: name,
            ident: ident,
            field_accessor: field_accessor,
            spec: spec,
        })
    }

    fn translate_endpoint<T>(
        &self,
        translator: &T,
        e: core::RpEndpoint<CoreFlavor>,
    ) -> Result<JavaEndpoint<'static>>
    where
        T: Translator<Source = CoreFlavor, Target = JavaFlavor>,
    {
        let ident = Cons::from(self.to_lower_camel.convert(e.safe_ident()));

        let name = {
            let ident = e.ident;
            e.name.map(Cons::from).unwrap_or_else(|| Cons::from(ident))
        };

        let response = e.response.translate(translator)?;
        let request = e.request.translate(translator)?;

        let response_ty = response.as_ref().map(|r| r.ty()).unwrap_or(&VOID).clone();
        let request_ty = request
            .as_ref()
            .map(|r| r.channel.ty())
            .unwrap_or(&VOID)
            .clone();

        let mut arguments = Vec::new();

        for arg in e.arguments {
            let ty = translator.translate_type(arg.channel.ty().clone())?;
            arguments.push(Argument::new(ty, arg.safe_ident().to_string()));
        }

        Ok(JavaEndpoint {
            ident,
            name,
            response,
            response_ty,
            request,
            request_ty,
            arguments: arguments,
            comment: e.comment,
            http: e.http.translate(translator)?,
        })
    }
}

decl_flavor!(JavaFlavor, core);
