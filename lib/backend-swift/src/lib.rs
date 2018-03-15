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
mod swift;

use backend::{Initializer, IntoBytes};
use compiler::Compiler;
use core::{Context, RpField, RpVersionedPackage};
use core::errors::Result;
use genco::Tokens;
use manifest::{Lang, Manifest, NoModule, TryFromToml};
use naming::Naming;
use std::any::Any;
use std::collections::BTreeMap;
use std::path::Path;
use std::rc::Rc;
use swift::Swift;
use trans::Environment;

const EXT: &str = "swift";

#[derive(Clone, Copy, Default, Debug)]
pub struct SwiftLang;

impl Lang for SwiftLang {
    lang_base!(SwiftModule, compile);

    fn comment(&self, input: &str) -> Option<String> {
        Some(format!("// {}", input))
    }

    fn package_naming(&self) -> Option<Box<Naming>> {
        Some(Box::new(naming::to_upper_camel()))
    }

    fn keywords(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("as", "as_"),
            ("associatedtype", "associatedtype_"),
            ("associativity", "associativity_"),
            ("break", "break_"),
            ("case", "case_"),
            ("catch", "catch_"),
            ("class", "class_"),
            ("continue", "continue_"),
            ("convenience", "convenience_"),
            ("default", "default_"),
            ("defer", "defer_"),
            ("deinit", "deinit_"),
            ("do", "do_"),
            ("dynamic", "dynamic_"),
            ("else", "else_"),
            ("enum", "enum_"),
            ("extension", "extension_"),
            ("fallthrough", "fallthrough_"),
            ("false", "false_"),
            ("fileprivate", "fileprivate_"),
            ("final", "final_"),
            ("for", "for_"),
            ("func", "func_"),
            ("get", "get_"),
            ("guard", "guard_"),
            ("if", "if_"),
            ("import", "import_"),
            ("in", "in_"),
            ("indirect", "indirect_"),
            ("infix", "infix_"),
            ("init", "init_"),
            ("inout", "inout_"),
            ("internal", "internal_"),
            ("is", "is_"),
            ("lazy", "lazy_"),
            ("left", "left_"),
            ("let", "let_"),
            ("mutating", "mutating_"),
            ("nil", "nil_"),
            ("none", "none_"),
            ("nonmutating", "nonmutating_"),
            ("open", "open_"),
            ("operator", "operator_"),
            ("optional", "optional_"),
            ("override", "override_"),
            ("postfix", "postfix_"),
            ("precedence", "precedence_"),
            ("prefix", "prefix_"),
            ("private", "private_"),
            ("protocol", "protocol_"),
            ("public", "public_"),
            ("repeat", "repeat_"),
            ("required", "required_"),
            ("rethrows", "rethrows_"),
            ("return", "return_"),
            ("right", "right_"),
            ("self", "self_"),
            ("set", "set_"),
            ("static", "static_"),
            ("struct", "struct_"),
            ("subscript", "subscript_"),
            ("super", "super_"),
            ("switch", "switch_"),
            ("throw", "throw_"),
            ("throws", "throws_"),
            ("true", "true_"),
            ("try", "try_"),
            ("typealias", "typealias_"),
            ("unowned", "unowned_"),
            ("var", "var_"),
            ("weak", "weak_"),
            ("where", "where_"),
            ("while", "while_"),
        ]
    }
}

#[derive(Debug)]
pub enum SwiftModule {
    Grpc,
    Simple,
    Codable,
}

impl TryFromToml for SwiftModule {
    fn try_from_string(path: &Path, id: &str, value: String) -> Result<Self> {
        use self::SwiftModule::*;

        let result = match id {
            "grpc" => Grpc,
            "simple" => Simple,
            "codable" => Codable,
            _ => return NoModule::illegal(path, id, value),
        };

        Ok(result)
    }

    fn try_from_value(path: &Path, id: &str, value: toml::Value) -> Result<Self> {
        use self::SwiftModule::*;

        let result = match id {
            "grpc" => Grpc,
            "simple" => Simple,
            "codable" => Codable,
            _ => return NoModule::illegal(path, id, value),
        };

        Ok(result)
    }
}

pub struct Options {
    /// All types that the struct model should extend.
    pub struct_model_extends: Tokens<'static, Swift<'static>>,
    pub type_gens: Vec<Box<TypeCodegen>>,
    pub tuple_gens: Vec<Box<TupleCodegen>>,
    pub struct_model_gens: Vec<Box<StructModelCodegen>>,
    pub enum_gens: Vec<Box<EnumCodegen>>,
    pub interface_gens: Vec<Box<InterfaceCodegen>>,
    pub interface_model_gens: Vec<Box<InterfaceModelCodegen>>,
    pub package_gens: Vec<Box<PackageCodegen>>,
    /// The provided Any type that should be used in structs.
    pub any_type: Vec<(&'static str, Tokens<'static, Swift<'static>>)>,
}

impl Options {
    pub fn new() -> Options {
        Options {
            struct_model_extends: Tokens::new(),
            type_gens: Vec::new(),
            tuple_gens: Vec::new(),
            struct_model_gens: Vec::new(),
            interface_gens: Vec::new(),
            interface_model_gens: Vec::new(),
            enum_gens: Vec::new(),
            package_gens: Vec::new(),
            any_type: Vec::new(),
        }
    }
}

pub fn options(modules: Vec<SwiftModule>) -> Result<Options> {
    use self::SwiftModule::*;

    let mut options = Options::new();

    for m in modules {
        debug!("+module: {:?}", m);

        let initializer: Box<Initializer<Options = Options>> = match m {
            Grpc => Box::new(module::Grpc::new()),
            Simple => Box::new(module::Simple::new()),
            Codable => Box::new(module::Codable::new()),
        };

        initializer.initialize(&mut options)?;
    }

    Ok(options)
}

pub struct FileSpec<'a>(pub Tokens<'a, Swift<'a>>);

impl<'el> Default for FileSpec<'el> {
    fn default() -> Self {
        FileSpec(Tokens::new())
    }
}

impl<'el> IntoBytes<Compiler<'el>> for FileSpec<'el> {
    fn into_bytes(self, _: &Compiler<'el>) -> Result<Vec<u8>> {
        let out = self.0.join_line_spacing().to_file()?;
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

/// Event emitted when a struct has been added.
pub struct TypeAdded<'a, 'c: 'a, 'el: 'a> {
    pub container: &'a mut Tokens<'el, Swift<'el>>,
    pub compiler: &'a Compiler<'c>,
    pub name: &'a Tokens<'el, Swift<'el>>,
    pub fields: &'a [&'el RpField],
}

codegen!(TypeCodegen, TypeAdded);

/// Event emitted when a struct has been added.
pub struct TupleAdded<'a, 'c: 'a, 'el: 'a> {
    pub container: &'a mut Tokens<'el, Swift<'el>>,
    pub compiler: &'a Compiler<'c>,
    pub name: &'a Tokens<'el, Swift<'el>>,
    pub fields: &'a [&'el RpField],
}

codegen!(TupleCodegen, TupleAdded);

/// Event emitted when a struct has been added.
pub struct StructModelAdded<'a, 'el: 'a> {
    pub container: &'a mut Tokens<'el, Swift<'el>>,
    pub fields: &'a [&'el RpField],
}

codegen!(StructModelCodegen, StructModelAdded);

/// Event emitted when an enum has been added.
pub struct EnumAdded<'a, 'el: 'a> {
    pub container: &'a mut Tokens<'el, Swift<'el>>,
    pub name: &'a Tokens<'el, Swift<'el>>,
    pub body: &'el core::RpEnumBody,
}

codegen!(EnumCodegen, EnumAdded);

/// Event emitted when an interface has been added.
pub struct InterfaceAdded<'a, 'c: 'a, 'el: 'a> {
    pub container: &'a mut Tokens<'el, Swift<'el>>,
    pub compiler: &'a Compiler<'c>,
    pub name: &'a Tokens<'el, Swift<'el>>,
    pub body: &'el core::RpInterfaceBody,
}

codegen!(InterfaceCodegen, InterfaceAdded);

/// Event emitted when an interface model has been added.
pub struct InterfaceModelAdded<'a, 'el: 'a> {
    pub container: &'a mut Tokens<'el, Swift<'el>>,
    pub body: &'el core::RpInterfaceBody,
}

codegen!(InterfaceModelCodegen, InterfaceModelAdded);

/// Event emitted when an interface model has been added.
pub struct PackageAdded<'a, 'el: 'a> {
    pub files: &'a mut BTreeMap<RpVersionedPackage, FileSpec<'el>>,
}

codegen!(PackageCodegen, PackageAdded);

fn compile(ctx: Rc<Context>, env: Environment, manifest: Manifest) -> Result<()> {
    let modules = manifest::checked_modules(manifest.modules)?;
    let options = options(modules)?;
    let handle = ctx.filesystem(manifest.output.as_ref().map(AsRef::as_ref))?;
    Compiler::new(&env, options, handle.as_ref())?.compile()
}
