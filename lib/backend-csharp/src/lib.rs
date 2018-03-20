#[macro_use]
extern crate genco;
#[macro_use]
extern crate log;
#[macro_use]
extern crate reproto_backend as backend;
extern crate reproto_core as core;
#[macro_use]
extern crate reproto_manifest as manifest;
extern crate reproto_naming as naming;
extern crate reproto_trans as trans;
extern crate serde;
#[allow(unused_imports)]
#[macro_use]
extern crate serde_derive;
extern crate toml;

mod processor;
mod compiler;
mod options;
mod csharp_file;
mod csharp_field;
mod codegen;
mod utils;
mod module;

use codegen::Configure;
use compiler::Compiler;
use core::Context;
use core::errors::Result;
use manifest::{Lang, Manifest, NoModule, TryFromToml, checked_modules};
use naming::Naming;
use options::Options;
use std::any::Any;
use std::path::Path;
use std::rc::Rc;
use trans::Environment;
use utils::Utils;

#[derive(Clone, Copy, Default, Debug)]
pub struct CsharpLang;

impl Lang for CsharpLang {
    lang_base!(CsharpModule, compile);

    fn comment(&self, input: &str) -> Option<String> {
        Some(format!("// {}", input))
    }

    fn package_naming(&self) -> Option<Box<Naming>> {
        Some(Box::new(naming::to_upper_camel()))
    }

    fn keywords(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("abstract", "_abstract"),
            ("as", "_as"),
            ("base", "_base"),
            ("bool", "_bool"),
            ("break", "_break"),
            ("byte", "_byte"),
            ("case", "_case"),
            ("catch", "_catch"),
            ("char", "_char"),
            ("checked", "_checked"),
            ("class", "_class"),
            ("const", "_const"),
            ("continue", "_continue"),
            ("decimal", "_decimal"),
            ("default", "_default"),
            ("delegate", "_delegate"),
            ("do", "_do"),
            ("double", "_double"),
            ("else", "_else"),
            ("enum", "_enum"),
            ("event", "_event"),
            ("explicit", "_explicit"),
            ("extern", "_extern"),
            ("false", "_false"),
            ("finally", "_finally"),
            ("fixed", "_fixed"),
            ("float", "_float"),
            ("for", "_for"),
            ("foreach", "_foreach"),
            ("goto", "_goto"),
            ("if", "_if"),
            ("implicit", "_implicit"),
            ("in", "_in"),
            ("int", "_int"),
            ("interface", "_interface"),
            ("internal", "_internal"),
            ("is", "_is"),
            ("lock", "_lock"),
            ("long", "_long"),
            ("namespace", "_namespace"),
            ("new", "_new"),
            ("null", "_null"),
            ("object", "_object"),
            ("operator", "_operator"),
            ("out", "_out"),
            ("override", "_override"),
            ("params", "_params"),
            ("private", "_private"),
            ("protected", "_protected"),
            ("public", "_public"),
            ("readonly", "_readonly"),
            ("ref", "_ref"),
            ("return", "_return"),
            ("sbyte", "_sbyte"),
            ("sealed", "_sealed"),
            ("short", "_short"),
            ("sizeof", "_sizeof"),
            ("stackalloc", "_stackalloc"),
            ("static", "_static"),
            ("string", "_string"),
            ("struct", "_struct"),
            ("switch", "_switch"),
            ("this", "_this"),
            ("throw", "_throw"),
            ("true", "_true"),
            ("try", "_try"),
            ("typeof", "_typeof"),
            ("uint", "_uint"),
            ("ulong", "_ulong"),
            ("unchecked", "_unchecked"),
            ("unsafe", "_unsafe"),
            ("ushort", "_ushort"),
            ("using", "_using"),
            ("virtual", "_virtual"),
            ("void", "_void"),
            ("volatile", "_volatile"),
            ("while", "_while"),
        ]
    }

    fn safe_packages(&self) -> bool {
        // NB: C# packages are upper-camel case, no keyword escaping needed.
        false
    }
}

#[derive(Debug)]
pub enum CsharpModule {
    JsonNet,
}

impl TryFromToml for CsharpModule {
    fn try_from_string(path: &Path, id: &str, value: String) -> Result<Self> {
        use self::CsharpModule::*;

        let result = match id {
            "Json.NET" => JsonNet,
            _ => return NoModule::illegal(path, id, value),
        };

        Ok(result)
    }

    fn try_from_value(path: &Path, id: &str, value: toml::Value) -> Result<Self> {
        use self::CsharpModule::*;

        let result = match id {
            "Json.NET" => JsonNet,
            _ => return NoModule::illegal(path, id, value),
        };

        Ok(result)
    }
}

fn setup_options<'a>(modules: Vec<CsharpModule>, utils: &Rc<Utils>) -> Options {
    use self::CsharpModule::*;

    let mut options = Options::new();

    for module in modules {
        let c = Configure {
            options: &mut options,
            utils: utils,
        };

        match module {
            JsonNet => module::JsonNet.initialize(c),
        };
    }

    options
}

fn compile(ctx: Rc<Context>, env: Environment, manifest: Manifest) -> Result<()> {
    let env = Rc::new(env);
    let utils = Rc::new(Utils::new(&env));
    let modules = checked_modules(manifest.modules)?;
    let options = setup_options(modules, &utils);
    let compiler = Compiler::new(&env, &utils, options);

    let handle = ctx.filesystem(manifest.output.as_ref().map(AsRef::as_ref))?;

    compiler.compile(handle.as_ref())
}
