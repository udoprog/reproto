//! Repository management commands.

use clap::{App, Arg, ArgMatches, SubCommand};
use core::Context;
use core::errors::*;
use repository::init_file_index;
use std::rc::Rc;

fn init(matches: &ArgMatches) -> Result<()> {
    for path in matches.values_of("path").into_iter().flat_map(|it| it) {
        info!("Creating repository: {}", path);
        init_file_index(path)?;
    }

    Ok(())
}

fn init_options<'a, 'b>() -> App<'a, 'b> {
    let out = SubCommand::with_name("init").about("Initialize a new repository");

    let out = out.arg(
        Arg::with_name("path")
            .required(true)
            .help("Path to repository to initialize"),
    );

    out
}

pub fn options<'a, 'b>() -> App<'a, 'b> {
    let out = SubCommand::with_name("repo").about("Manage repositories");
    let out = out.subcommand(init_options());
    out
}

pub fn entry(_ctx: Rc<Context>, matches: &ArgMatches) -> Result<()> {
    let (name, matches) = matches.subcommand();
    let matches = matches.ok_or_else(|| "no subcommand")?;

    match name {
        "init" => init(matches),
        _ => unreachable!("bad subcommand"),
    }
}
