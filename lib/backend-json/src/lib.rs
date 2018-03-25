#[macro_use]
extern crate log;
#[allow(unused)]
#[macro_use]
extern crate reproto_backend as backend;
extern crate reproto_core as core;
#[macro_use]
extern crate reproto_manifest as manifest;
extern crate reproto_trans as trans;
extern crate serde;
extern crate serde_json;
extern crate toml;

use core::errors::*;
use core::{Context, CoreFlavor, RelativePathBuf};
use manifest::{Lang, Manifest, NoModule, TryFromToml};
use std::any::Any;
use std::path::Path;
use std::rc::Rc;
use trans::Environment;

#[derive(Clone, Copy, Default, Debug)]
pub struct JsonLang;

impl Lang for JsonLang {
    lang_base!(JsonModule, compile);
}

#[derive(Debug)]
pub enum JsonModule {
}

impl TryFromToml for JsonModule {
    fn try_from_string(path: &Path, id: &str, value: String) -> Result<Self> {
        NoModule::illegal(path, id, value)
    }

    fn try_from_value(path: &Path, id: &str, value: toml::Value) -> Result<Self> {
        NoModule::illegal(path, id, value)
    }
}

fn compile(ctx: Rc<Context>, env: Environment<CoreFlavor>, manifest: Manifest) -> Result<()> {
    let env = env.translate_default()?;
    let handle = ctx.filesystem(manifest.output.as_ref().map(AsRef::as_ref))?;

    let root = RelativePathBuf::from(".");

    for (package, file) in env.for_each_file() {
        let mut path = package
            .package
            .parts
            .iter()
            .fold(root.clone(), |path, part| path.join(part));

        let parent = path.parent()
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| root.clone());

        if !handle.is_dir(&parent) {
            debug!("+dir: {}", parent.display());
            handle.create_dir_all(&parent)?;
        }

        let path = if let Some(version) = package.version.as_ref() {
            let stem = path.file_stem()
                .ok_or_else(|| format!("Missing file stem: {}", path.display()))?;

            let file_name = format!("{}-{}.json", stem, version);
            path.with_file_name(file_name)
        } else {
            path.with_extension("json")
        };

        debug!("+file: {}", path.display());
        writeln!(
            handle.create(&path)?,
            "{}",
            serde_json::to_string_pretty(file)?,
        )?;
    }

    Ok(())
}
