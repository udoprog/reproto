//! build command

use build_spec::{environment, load_manifest};
use clap::{App, Arg, ArgMatches, SubCommand};
use core::Context;
use core::errors::Result;
use std::rc::Rc;

pub fn options<'a, 'b>() -> App<'a, 'b> {
    let out = SubCommand::with_name("build").about("Build specifications");

    let out = out.arg(
        Arg::with_name("lang")
            .long("lang")
            .takes_value(true)
            .help("Language to build for"),
    );

    out
}

pub fn entry(ctx: Rc<Context>, matches: &ArgMatches) -> Result<()> {
    let manifest = load_manifest(matches)?;
    let lang = manifest.lang().ok_or_else(|| {
        "no language to build for, either specify in manifest under `language` or `--lang`"
    })?;
    let env = environment(ctx.clone(), lang.copy(), &manifest)?;

    lang.compile(ctx, env, manifest)?;
    Ok(())
}
