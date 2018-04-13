//! Action to build documentation.

use build_spec::simple_config;
use clap::{App, ArgMatches, SubCommand};
use core::Context;
use core::errors::*;
use std::rc::Rc;

pub fn options<'a, 'b>() -> App<'a, 'b> {
    ::doc::shared_options(SubCommand::with_name("doc").about("Generate documentation"))
}

pub fn entry(ctx: Rc<Context>, matches: &ArgMatches) -> Result<()> {
    let (manifest, env) = simple_config(&ctx, matches)?;
    ::doc::compile(env, matches, manifest).map_err(Into::into)
}
