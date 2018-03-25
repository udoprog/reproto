#[macro_use]
extern crate genco;
#[macro_use]
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

#[macro_use]
mod utils;
mod compiler;

use backend::IntoBytes;
use compiler::Compiler;
use core::errors::Result;
use core::{Context, CoreFlavor, Loc, Pos, RpField, RpPackage, RpType};
use genco::{JavaScript, Tokens};
use manifest::{Lang, Manifest, NoModule, TryFromToml};
use std::any::Any;
use std::path::Path;
use std::rc::Rc;
use trans::Environment;

const TYPE_SEP: &str = "_";
const EXT: &str = "js";

#[derive(Clone, Copy, Default, Debug)]
pub struct JsLang;

impl Lang for JsLang {
    lang_base!(JsModule, compile);

    fn comment(&self, input: &str) -> Option<String> {
        Some(format!("# {}", input))
    }

    fn keywords(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("abstract", "_abstract"),
            ("await", "_await"),
            ("boolean", "_boolean"),
            ("break", "_break"),
            ("byte", "_byte"),
            ("case", "_case"),
            ("catch", "_catch"),
            ("char", "_char"),
            ("class", "_class"),
            ("const", "_const"),
            ("continue", "_continue"),
            ("debugger", "_debugger"),
            ("default", "_default"),
            ("delete", "_delete"),
            ("do", "_do"),
            ("double", "_double"),
            ("else", "_else"),
            ("enum", "_enum"),
            ("export", "_export"),
            ("extends", "_extends"),
            ("false", "_false"),
            ("final", "_final"),
            ("finally", "_finally"),
            ("float", "_float"),
            ("for", "_for"),
            ("function", "_function"),
            ("goto", "_goto"),
            ("if", "_if"),
            ("implements", "_implements"),
            ("import", "_import"),
            ("in", "_in"),
            ("instanceof", "_instanceof"),
            ("int", "_int"),
            ("interface", "_interface"),
            ("let", "_let"),
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
            ("super", "_super"),
            ("switch", "_switch"),
            ("synchronized", "_synchronized"),
            ("this", "_this"),
            ("throw", "_throw"),
            ("throws", "_throws"),
            ("transient", "_transient"),
            ("true", "_true"),
            ("try", "_try"),
            ("typeof", "_typeof"),
            ("var", "_var"),
            ("void", "_void"),
            ("volatile", "_volatile"),
            ("while", "_while"),
            ("with", "_with"),
            ("yield", "_yield"),
        ]
    }

    fn safe_packages(&self) -> bool {
        // NB: JavaScript imports by string literals, no keyword escaping needed.
        false
    }
}

#[derive(Debug)]
pub enum JsModule {
}

impl TryFromToml for JsModule {
    fn try_from_string(path: &Path, id: &str, value: String) -> Result<Self> {
        NoModule::illegal(path, id, value)
    }

    fn try_from_value(path: &Path, id: &str, value: toml::Value) -> Result<Self> {
        NoModule::illegal(path, id, value)
    }
}

pub struct Options {
    pub build_getters: bool,
    pub build_constructor: bool,
}

impl Options {
    pub fn new() -> Options {
        Options {
            build_getters: false,
            build_constructor: true,
        }
    }
}

pub struct FileSpec<'el>(pub Tokens<'el, JavaScript<'el>>);

impl<'el> Default for FileSpec<'el> {
    fn default() -> Self {
        FileSpec(Tokens::new())
    }
}

impl<'el> IntoBytes<Compiler<'el>> for FileSpec<'el> {
    fn into_bytes(self, _: &Compiler<'el>, _: &RpPackage) -> Result<Vec<u8>> {
        let out = self.0.join_line_spacing().to_file()?;
        Ok(out.into_bytes())
    }
}

fn compile(ctx: Rc<Context>, env: Environment<CoreFlavor>, manifest: Manifest) -> Result<()> {
    let env = env.translate_default()?;
    let _modules: Vec<JsModule> = manifest::checked_modules(manifest.modules)?;
    let options = Options::new();
    let handle = ctx.filesystem(manifest.output.as_ref().map(AsRef::as_ref))?;
    let variant_field = Loc::new(RpField::new("value", RpType::String), Pos::empty());

    Compiler::new(&env, &variant_field, options, handle.as_ref()).compile()
}
