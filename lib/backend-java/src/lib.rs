#[macro_use]
extern crate genco;
#[macro_use]
extern crate log;
extern crate reproto_backend as backend;
#[macro_use]
extern crate reproto_core as core;
#[macro_use]
extern crate reproto_manifest as manifest;
extern crate reproto_naming as naming;
extern crate reproto_trans as trans;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate toml;

mod codegen;
mod compiler;
mod flavored;
mod java_file;
mod module;
mod options;
mod processor;
mod utils;

use codegen::Configure;
use compiler::Compiler;
use core::errors::Result;
use core::{Context, CoreFlavor, Loc, Pos, RpField, RpType, Translator};
use manifest::{checked_modules, Lang, Manifest, NoModule, TryFromToml};
use options::Options;
use std::any::Any;
use std::path::Path;
use std::rc::Rc;
use trans::Environment;

#[derive(Clone, Copy, Default, Debug)]
pub struct JavaLang;

impl Lang for JavaLang {
    lang_base!(JavaModule, compile);

    fn comment(&self, input: &str) -> Option<String> {
        Some(format!("// {}", input))
    }

    fn keywords(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("abstract", "_abstract"),
            ("assert", "_assert"),
            ("boolean", "_boolean"),
            ("break", "_break"),
            ("byte", "_byte"),
            ("case", "_case"),
            ("catch", "_catch"),
            ("char", "_char"),
            ("class", "_class"),
            ("const", "_const"),
            ("continue", "_continue"),
            ("default", "_default"),
            ("do", "_do"),
            ("double", "_double"),
            ("else", "_else"),
            ("enum", "_enum"),
            ("extends", "_extends"),
            ("false", "_false"),
            ("final", "_final"),
            ("finally", "_finally"),
            ("float", "_float"),
            ("for", "_for"),
            ("goto", "_goto"),
            ("if", "_if"),
            ("implements", "_implements"),
            ("import", "_import"),
            ("instanceof", "_instanceof"),
            ("int", "_int"),
            ("interface", "_interface"),
            ("long", "_long"),
            ("native", "_native"),
            ("new", "_new"),
            ("null", "_null"),
            ("package", "_package"),
            ("private", "_private"),
            ("protected", "_protected"),
            ("public", "_public"),
            ("return", "_return"),
            ("short", "_short"),
            ("static", "_static"),
            ("strictfp", "_strictfp"),
            ("super", "_super"),
            ("switch", "_switch"),
            ("synchronized", "_synchronized"),
            ("this", "_this"),
            ("throw", "_throw"),
            ("throws", "_throws"),
            ("transient", "_transient"),
            ("true", "_true"),
            ("try", "_try"),
            ("void", "_void"),
            ("volatile", "_volatile"),
            ("while", "_while"),
        ]
    }
}

#[derive(Debug)]
pub enum JavaModule {
    Jackson,
    Lombok,
    Grpc,
    Builder,
    ConstructorProperties,
    Mutable,
    Nullable,
    OkHttp(module::OkHttpConfig),
}

impl TryFromToml for JavaModule {
    fn try_from_string(path: &Path, id: &str, value: String) -> Result<Self> {
        use self::JavaModule::*;

        let result = match id {
            "jackson" => Jackson,
            "lombok" => Lombok,
            "grpc" => Grpc,
            "builder" => Builder,
            "constructor_properties" => ConstructorProperties,
            "mutable" => Mutable,
            "nullable" => Nullable,
            "okhttp" => OkHttp(module::OkHttpConfig::default()),
            _ => return NoModule::illegal(path, id, value),
        };

        Ok(result)
    }

    fn try_from_value(path: &Path, id: &str, value: toml::Value) -> Result<Self> {
        use self::JavaModule::*;

        let result = match id {
            "jackson" => Jackson,
            "lombok" => Lombok,
            "grpc" => Grpc,
            "builder" => Builder,
            "constructor_properties" => ConstructorProperties,
            "mutable" => Mutable,
            "nullable" => Nullable,
            "okhttp" => OkHttp(value.try_into()?),
            _ => return NoModule::illegal(path, id, value),
        };

        Ok(result)
    }
}

fn setup_options<'a>(modules: Vec<JavaModule>) -> Options {
    use self::JavaModule::*;

    let mut options = Options::new();

    for module in modules {
        let c = Configure {
            options: &mut options,
        };

        match module {
            Jackson => module::Jackson.initialize(c),
            Lombok => module::Lombok.initialize(c),
            Grpc => module::Grpc.initialize(c),
            Builder => module::Builder.initialize(c),
            ConstructorProperties => module::ConstructorProperties.initialize(c),
            Mutable => module::Mutable.initialize(c),
            Nullable => module::Nullable.initialize(c),
            OkHttp(config) => module::OkHttp::new(config).initialize(c),
        };
    }

    options
}

fn compile(ctx: Rc<Context>, env: Environment<CoreFlavor>, manifest: Manifest) -> Result<()> {
    let translator = env.translator(flavored::JavaTypeTranslator::new());

    let variant_field = Loc::new(
        translator.translate_field(RpField::new("value", RpType::String))?,
        Pos::empty(),
    );

    let env = env.translate(translator)?;

    let env = Rc::new(env);
    let modules = checked_modules(manifest.modules)?;
    let options = setup_options(modules);

    let compiler = Compiler::new(&env, &variant_field, options);

    let handle = ctx.filesystem(manifest.output.as_ref().map(AsRef::as_ref))?;

    compiler.compile(handle.as_ref())
}
