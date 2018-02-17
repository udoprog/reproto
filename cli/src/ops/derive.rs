//! Derive a schema from the given input.

use clap::{App, Arg, ArgMatches, SubCommand};
use core::{Context, PathObject, StdinObject};
use derive;
use errors::Result;
use std::path::Path;
use std::rc::Rc;

pub fn options<'a, 'b>() -> App<'a, 'b> {
    let out = SubCommand::with_name("derive").about("Derive a schema from the given input");

    let out = out.arg(
        Arg::with_name("file")
            .long("file")
            .short("i")
            .takes_value(true)
            .help("File to read from, otherwise will read from stdin"),
    );

    out
}

pub fn entry(_ctx: Rc<Context>, matches: &ArgMatches) -> Result<()> {
    match matches.value_of("file") {
        Some(file) => derive::derive(PathObject::new(None, Path::new(file)))?,
        None => {
            derive::derive(StdinObject::new())?;
        }
    }

    Ok(())
}
