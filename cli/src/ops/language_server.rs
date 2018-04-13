//! Update action that synchronizes all repositories.

#[cfg(feature = "languageserver")]
extern crate reproto_languageserver as ls;

use clap::{App, Arg, ArgMatches, SubCommand};
use core::Context;
use core::errors::Result;
use std::fs;
use std::io;
use std::rc::Rc;

pub fn options<'a, 'b>() -> App<'a, 'b> {
    let out = SubCommand::with_name("language-server").about("Run the language server for reproto");

    let out = out.arg(
        Arg::with_name("lang")
            .long("lang")
            .takes_value(true)
            .help("Language to build for"),
    );

    let out = out.arg(
        Arg::with_name("log")
            .long("log")
            .takes_value(true)
            .help("Log to the given path"),
    );

    out
}

pub fn entry(_ctx: Rc<Context>, matches: &ArgMatches) -> Result<()> {
    #[cfg(feature = "languageserver")]
    {
        let mut log = match matches.value_of("log") {
            Some(log) => Some(fs::File::create(log)?),
            None => None,
        };

        ls::server(&mut log, io::stdin(), io::stdout())?;
        Ok(())
    }

    #[cfg(not(feature = "languageserver"))]
    {
        Err("languageserver feature is not enabled".into())
    }
}
