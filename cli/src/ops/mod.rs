mod build;
mod check;
mod derive;
mod doc;
mod init;
mod publish;
mod repo;
mod update;
mod watch;

use clap::{App, Arg, ArgMatches};
use core::Context;
use core::errors::*;
use output::Output;
use std::rc::Rc;

pub fn base_args<'a, 'b>(out: App<'a, 'b>) -> App<'a, 'b> {
    let out = out.arg(
        Arg::with_name("index")
            .long("index")
            .short("I")
            .takes_value(true)
            .help("URL for index to use when looking up packages."),
    );

    let out = out.arg(
        Arg::with_name("no-repository")
            .long("no-repository")
            .takes_value(false)
            .help("Completely disable repository operations"),
    );

    let out = out.arg(
        Arg::with_name("objects")
            .long("objects")
            .short("O")
            .takes_value(true)
            .help("URL for objects storage to use when looking up packages."),
    );

    let out = out.arg(
        Arg::with_name("path")
            .long("path")
            .short("p")
            .takes_value(true)
            .multiple(true)
            .number_of_values(1)
            .help("Paths to look for definitions."),
    );

    let out = out.arg(
        Arg::with_name("manifest-path")
            .long("manifest-path")
            .takes_value(true)
            .help("Path to manifest to build"),
    );

    out
}

/// Setup base compiler options.
pub fn build_args<'a, 'b>(out: App<'a, 'b>) -> App<'a, 'b> {
    let out = base_args(out);

    let out = out.arg(
        Arg::with_name("package")
            .long("package")
            .help("Packages to compile")
            .takes_value(true)
            .multiple(true)
            .number_of_values(1),
    );

    let out = out.arg(
        Arg::with_name("module")
            .long("module")
            .short("m")
            .takes_value(true)
            .multiple(true)
            .number_of_values(1)
            .help("Modules to load for a given backend"),
    );

    let out = out.arg(
        Arg::with_name("id-converter")
            .long("id-converter")
            .takes_value(true)
            .help("Conversion method to use when naming fields by default"),
    );

    let out = out.arg(
        Arg::with_name("package-prefix")
            .long("package-prefix")
            .takes_value(true)
            .help("Package prefix to use when generating classes"),
    );

    let out = out.arg(
        Arg::with_name("file")
            .long("file")
            .help("File to compile")
            .takes_value(true)
            .multiple(true)
            .number_of_values(1),
    );

    let out = out.arg(
        Arg::with_name("out")
            .long("out")
            .short("o")
            .takes_value(true)
            .help("Output directory"),
    );

    out
}

pub fn options<'a, 'b>(out: App<'a, 'b>) -> App<'a, 'b> {
    let out = out.subcommand(build_args(build::options()));
    let out = out.subcommand(build_args(doc::options()));
    let out = out.subcommand(build_args(watch::options()));
    let out = out.subcommand(base_args(check::options()));
    let out = out.subcommand(base_args(publish::options()));
    let out = out.subcommand(base_args(update::options()));
    let out = out.subcommand(base_args(repo::options()));
    let out = out.subcommand(derive::options());
    let out = out.subcommand(init::options());
    out
}

pub fn entry(ctx: Rc<Context>, matches: &ArgMatches, output: &Output) -> Result<()> {
    let (name, matches) = matches.subcommand();
    let matches = matches.ok_or_else(|| "no subcommand")?;

    match name {
        "build" => return build::entry(ctx, matches),
        "check" => return check::entry(ctx, matches),
        "derive" => return derive::entry(ctx, matches),
        "doc" => return doc::entry(ctx, matches),
        "init" => return init::entry(ctx, matches),
        "publish" => return publish::entry(ctx, matches),
        "repo" => return repo::entry(ctx, matches),
        "update" => return update::entry(ctx, matches),
        "watch" => return watch::entry(ctx, matches, output),
        _ => {}
    }

    Err(format!("No such command: {}", name).into())
}
