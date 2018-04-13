extern crate atty;
extern crate clap;
extern crate log;
extern crate reproto;
extern crate reproto_core as core;

use clap::{App, Arg, ArgMatches};
use core::errors::Result;
use core::{Context, RealFilesystem};
use reproto::{ops, output, VERSION};
use std::io;
use std::rc::Rc;

fn setup_opts<'a, 'b>() -> App<'a, 'b> {
    App::new("reproto")
        .version(VERSION)
        .arg(
            Arg::with_name("debug")
                .long("debug")
                .short("D")
                .help("Enable debug output."),
        )
        .arg(
            Arg::with_name("color")
                .long("color")
                .help("Force colored output."),
        )
        .arg(
            Arg::with_name("no-color")
                .long("no-color")
                .help("Disable colored output."),
        )
        .arg(
            Arg::with_name("output-format")
                .long("output-format")
                .takes_value(true)
                .help("Select a different output format (json, human) (default: human)."),
        )
}

/// Configure logging
///
/// If debug (--debug) is specified, logging should be configured with `LogLevelFilter::Debug`.
fn setup_logger(matches: &clap::ArgMatches, output: &output::Output) -> Result<()> {
    let level = if matches.is_present("debug") {
        log::LogLevelFilter::Debug
    } else {
        log::LogLevelFilter::Info
    };

    log::set_logger(|max_level| {
        max_level.set(level);
        output.logger()
    })?;

    Ok(())
}

fn guarded_entry(ctx: Rc<Context>, matches: &ArgMatches, output: &output::Output) -> Result<()> {
    setup_logger(matches, output)?;
    ops::entry(ctx, matches, output)?;
    Ok(())
}

fn entry(matches: &ArgMatches, output: &output::Output) -> Result<()> {
    let ctx = Rc::new(Context::new(Box::new(RealFilesystem::new())));

    match guarded_entry(Rc::clone(&ctx), matches, output) {
        Err(e) => {
            // NB: get rid of dual reporting.
            // We only want positional errors reported through the context.
            if !ctx.has_diagnostics()? {
                output.handle_error(&e)?;
            }

            output.handle_context(ctx.items()?.as_ref())?;
            ::std::process::exit(1);
        }
        Ok(()) => {
            let ctx_items = ctx.items()?;
            output.handle_context(ctx_items.as_ref())?;
        }
    }

    Ok(())
}

fn main() {
    let opts = setup_opts();
    let opts = ops::options(opts);
    let matches = opts.get_matches();

    let colored = matches.is_present("color")
        || !matches.is_present("no-color") && atty::is(atty::Stream::Stdout);

    let output_format = match matches.value_of("output-format") {
        Some("json") => output::OutputFormat::Json,
        _ => output::OutputFormat::Human,
    };

    let mut output: Box<output::Output> = match output_format {
        output::OutputFormat::Json => Box::new(output::Json::new(io::stdout())),
        _ if colored => Box::new(output::Colored::new(io::stdout())),
        _ => Box::new(output::NonColored::new(io::stdout())),
    };

    if let Err(e) = entry(&matches, output.as_mut()) {
        output.error(&e).unwrap();
        ::std::process::exit(1);
    }

    ::std::process::exit(0);
}
