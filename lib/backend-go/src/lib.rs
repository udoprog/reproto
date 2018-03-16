#[macro_use]
extern crate genco;
#[macro_use]
extern crate log;
extern crate reproto_backend as backend;
extern crate reproto_core as core;
#[macro_use]
extern crate reproto_manifest as manifest;
extern crate reproto_naming as naming;
extern crate reproto_trans as trans;
extern crate serde;
#[allow(unused)]
#[macro_use]
extern crate serde_derive;
extern crate toml;

mod compiler;
mod module;
mod go;

use backend::{Initializer, IntoBytes};
use compiler::Compiler;
use core::{Context, RpField, RpPackage};
use core::errors::Result;
use genco::{Element, IntoTokens, Tokens};
use manifest::{Lang, Manifest, NoModule, TryFromToml};
use naming::Naming;
use std::any::Any;
use std::path::Path;
use std::rc::Rc;
use go::Go;
use trans::Environment;
use std::collections::HashMap;

const EXT: &str = "go";

#[derive(Clone, Copy, Default, Debug)]
pub struct GoLang;

impl Lang for GoLang {
    lang_base!(GoModule, compile);

    fn comment(&self, input: &str) -> Option<String> {
        Some(format!("// {}", input))
    }

    fn keywords(&self) -> Vec<(&'static str, &'static str)> {
        vec![]
    }

    fn field_ident_naming(&self) -> Option<Box<Naming>> {
        Some(Box::new(naming::to_upper_camel()))
    }
}

#[derive(Debug)]
pub enum GoModule {
    EncodingJson,
}

impl TryFromToml for GoModule {
    fn try_from_string(path: &Path, id: &str, value: String) -> Result<Self> {
        use self::GoModule::*;

        let result = match id {
            "encoding/json" => EncodingJson,
            _ => return NoModule::illegal(path, id, value),
        };

        Ok(result)
    }

    fn try_from_value(path: &Path, id: &str, value: toml::Value) -> Result<Self> {
        use self::GoModule::*;

        let result = match id {
            "encoding/json" => EncodingJson,
            _ => return NoModule::illegal(path, id, value),
        };

        Ok(result)
    }
}

pub struct Options {
    pub field_gens: Vec<Box<FieldCodegen>>,
    pub enum_gens: Vec<Box<EnumCodegen>>,
    pub tuple_gens: Vec<Box<TupleCodegen>>,
    pub interface_gens: Vec<Box<InterfaceCodegen>>,
}

impl Options {
    pub fn new() -> Options {
        Options {
            field_gens: Vec::new(),
            enum_gens: Vec::new(),
            tuple_gens: Vec::new(),
            interface_gens: Vec::new(),
        }
    }
}

pub fn options(modules: Vec<GoModule>) -> Result<Options> {
    use self::GoModule::*;

    let mut options = Options::new();

    for m in modules {
        debug!("+module: {:?}", m);

        let initializer: Box<Initializer<Options = Options>> = match m {
            EncodingJson => Box::new(module::EncodingJson::new()),
        };

        initializer.initialize(&mut options)?;
    }

    Ok(options)
}

pub struct FileSpec<'a>(pub Tokens<'a, Go<'a>>);

impl<'el> Default for FileSpec<'el> {
    fn default() -> Self {
        FileSpec(Tokens::new())
    }
}

impl<'el> IntoBytes<Compiler<'el>> for FileSpec<'el> {
    fn into_bytes(self, _: &Compiler<'el>, package: &RpPackage) -> Result<Vec<u8>> {
        let package = package
            .parts
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join("_");

        let extra = go::Extra::from_package(package);

        let out = self.0.join_line_spacing().to_file_with(extra)?;
        Ok(out.into_bytes())
    }
}

/// Build codegen hooks.
macro_rules! codegen {
    ($c:tt, $e:ty) => {
        pub trait $c {
            fn generate(&self, e: $e) -> Result<()>;
        }

        impl<T> $c for Rc<T> where T: $c {
            fn generate(&self, e: $e) -> Result<()> {
                self.as_ref().generate(e)
            }
        }
    }
}

/// Event emitted when a field has been added.
pub struct FieldAdded<'a, 'el: 'a> {
    pub tags: &'a mut Tags,
    pub field: &'el RpField,
}

codegen!(FieldCodegen, FieldAdded);

/// Event emitted when an enum has been added
pub struct EnumAdded<'a, 'el: 'a> {
    pub container: &'a mut Tokens<'el, Go<'el>>,
    pub name: Go<'el>,
    pub body: &'el core::RpEnumBody,
}

codegen!(EnumCodegen, EnumAdded);

/// Event emitted when a tuple has been added.
pub struct TupleAdded<'a, 'el: 'a> {
    pub container: &'a mut Tokens<'el, Go<'el>>,
    pub name: Go<'el>,
    pub body: &'el core::RpTupleBody,
    pub compiler: &'a Compiler<'el>,
}

codegen!(TupleCodegen, TupleAdded);

/// Event emitted when an interface has been added.
pub struct InterfaceAdded<'a, 'el: 'a> {
    pub container: &'a mut Tokens<'el, Go<'el>>,
    pub name: Go<'el>,
    pub body: &'el core::RpInterfaceBody,
    pub compiler: &'a Compiler<'el>,
}

codegen!(InterfaceCodegen, InterfaceAdded);

pub enum TagValue {
    String(String),
}

impl From<TagValue> for Element<'static, Go<'static>> {
    fn from(value: TagValue) -> Self {
        match value {
            TagValue::String(string) => Element::from(string),
        }
    }
}

/// Structure for Tags - a type of Go metadata
pub struct Tags {
    values: HashMap<String, Vec<TagValue>>,
}

impl Tags {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    /// Push a string tag.
    pub fn push_str<K: AsRef<str>, V: AsRef<str>>(&mut self, key: K, value: V) {
        self.values
            .entry(key.as_ref().to_string())
            .or_insert_with(Vec::new)
            .push(TagValue::String(value.as_ref().to_string()));
    }
}

impl<'el> IntoTokens<'el, Go<'el>> for Tags {
    fn into_tokens(self) -> Tokens<'el, Go<'el>> {
        let mut t = Tokens::new();

        if self.values.is_empty() {
            return t;
        }

        t.append("`");

        t.append({
            let mut t = Tokens::new();

            for (key, vals) in self.values {
                t.append({
                    let mut t = Tokens::new();
                    t.append(key);
                    t.append(":");

                    let vals = vals.into_iter()
                        .fold(Tokens::new(), |mut t, v| {
                            t.append(Element::from(v));
                            t
                        })
                        .join(",");

                    t.append("\"");
                    t.append(vals);
                    t.append("\"");

                    t
                });
            }

            t.join(" ")
        });

        t.append("`");

        t
    }
}

fn compile(ctx: Rc<Context>, env: Environment, manifest: Manifest) -> Result<()> {
    let modules = manifest::checked_modules(manifest.modules)?;
    let options = options(modules)?;
    let handle = ctx.filesystem(manifest.output.as_ref().map(AsRef::as_ref))?;
    Compiler::new(&env, options, handle.as_ref())?.compile()
}
