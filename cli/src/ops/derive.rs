//! Derive a schema from the given input.

use ast;
use clap::{App, Arg, ArgMatches, SubCommand};
use compile;
use core::errors::Result;
use core::{Context, RpPackage, RpVersionedPackage, Source};
use derive;
use env;
use genco::IoFmt;
use manifest::{Lang, Language};
use std::any::Any;
use std::fmt::Write;
use std::io;
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

    let out = out.arg(
        Arg::with_name("root-name")
            .long("root-name")
            .takes_value(true)
            .help("Name of the root object to generate"),
    );

    let out = out.arg(
        Arg::with_name("package-prefix")
            .long("package-prefix")
            .takes_value(true)
            .help("Package prefix to use"),
    );

    let out = out.arg(
        Arg::with_name("format")
            .long("format")
            .short("F")
            .takes_value(true)
            .help("Format to decode, valid values: json, yaml"),
    );

    let out = out.arg(
        Arg::with_name("lang")
            .long("lang")
            .takes_value(true)
            .help("Language to compile to"),
    );

    let out = out.arg(
        Arg::with_name("module")
            .long("module")
            .short("m")
            .takes_value(true)
            .help("Modules to enable"),
    );

    out
}

pub fn entry(_ctx: Rc<Context>, matches: &ArgMatches) -> Result<()> {
    let root_name = match matches.value_of("root-name") {
        None => "Generated".to_string(),
        Some(name) => name.to_string(),
    };

    let package_prefix = match matches.value_of("package-prefix") {
        None => RpPackage::parse("io.github.reproto"),
        Some(name) => RpPackage::parse(name),
    };

    let format: Box<derive::Format> = match matches.value_of("format") {
        None | Some("json") => Box::new(derive::Json),
        Some("yaml") => Box::new(derive::Yaml),
        Some(value) => return Err(format!("Unsupported format: {}", value).into()),
    };

    let source = match matches.value_of("file") {
        Some(file) => Source::from_path(file),
        None => Source::stdin(),
    };

    let derive = derive::Derive::new(root_name, format, Some(package_prefix.clone()));

    let decl = derive::derive(derive, &source)?;

    let file = ast::File {
        comment: vec!["Generated from reproto derive CLI".to_string().into()],
        attributes: vec![],
        uses: vec![],
        decls: vec![decl],
    };

    let input = compile::Input::File(
        file,
        Some(RpVersionedPackage::new(package_prefix.clone(), None)),
    );

    let stdout = io::stdout();

    let simple_compile = compile::SimpleCompile::new(input).package_prefix(package_prefix);

    let modules: Vec<String> = matches
        .values_of("module")
        .into_iter()
        .flat_map(|s| s.map(|s| s.to_string()))
        .collect();

    let language = matches
        .value_of("lang")
        .and_then(Language::parse)
        .ok_or_else(|| "no language specified, use `--lang`")?;

    let lang = env::convert_lang(language);

    let modules = load_modules(lang.as_ref(), modules)?;

    let mut stdout = stdout.lock();

    compile::simple_compile(
        |path, content| {
            let mut buf = IoFmt(&mut stdout);

            if let Some(comment) = lang.comment(format!(" File: {}", path.display()).as_str()) {
                writeln!(buf, "{}", comment)?;
                writeln!(buf, "")?;
            }

            buf.write_str(content)?;
            Ok(())
        },
        simple_compile,
        modules,
        lang.as_ref(),
    )?;

    return Ok(());

    fn load_modules(lang: &Lang, names: Vec<String>) -> Result<Vec<Box<Any>>> {
        let mut modules = Vec::new();

        for name in names {
            modules.push(lang.string_spec(Path::new("."), name.as_str())?);
        }

        Ok(modules)
    }
}
