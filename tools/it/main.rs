#[macro_use]
extern crate failure;
extern crate it;
extern crate rayon;

use it::{Action, Instance, Language, Project, Reproto, Result};
use rayon::prelude::*;
use std::collections::HashSet;
use std::env;
use std::path::PathBuf;
use std::process::{self, Command, Stdio};
use std::time::Instant;

/// Test if the given command successfully runs.
fn test(command: &str, args: &[&str]) -> bool {
    Command::new(command)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .args(args)
        .status()
        .is_ok()
}

/// Detect supported languages
fn detect() -> Vec<Language> {
    let mut out = vec![Language::Json, Language::Reproto];

    if test("mvn", &["--version"]) {
        out.push(Language::Java);
    }

    if test("python", &["--version"]) {
        out.push(Language::Python);
    }

    if test("python3", &["--version"]) {
        out.push(Language::Python3);
    }

    if test("cargo", &["--version"]) {
        out.push(Language::Rust);
    }

    if test("node", &["--version"]) && test("babel", &["--version"]) {
        out.push(Language::JavaScript);
    }

    if test("dotnet", &["--version"]) {
        out.push(Language::Csharp);
    }

    if test("swift", &["--version"]) {
        out.push(Language::Swift);
    }

    if test("go", &["version"]) {
        out.push(Language::Go);
    }

    out
}

fn try_main() -> Result<()> {
    let languages = detect();

    let mut root = env::current_dir()?;
    let mut args = env::args();
    args.next();

    let mut do_suite = false;
    let mut do_project = false;
    let mut action = Action::Verify;
    let mut filters = HashSet::new();

    while let Some(opt) = args.next() {
        match opt.as_str() {
            "--update" => {
                action = Action::Update;
            }
            "--suite" => {
                do_suite = true;
            }
            "--project" => {
                do_project = true;
            }
            "--root" => {
                let arg = args.next()
                    .ok_or_else(|| format_err!("expected argument to `--root`"))?;
                root = PathBuf::from(arg);
            }
            other => {
                filters.insert(other.to_string());
            }
        }
    }

    let parent = root.parent()
        .ok_or_else(|| format_err!("no parent directory"))?;

    let cli = parent.join("cli");

    let reproto = Reproto::from_project(cli)?;

    let mut project = Project::new(&languages, &reproto, do_suite, do_project, action);

    project.arg(Language::Go, &["-m", "encoding/json"]);
    project.arg(Language::Java, &["-m", "builder", "-m", "jackson"]);
    project.arg(Language::Csharp, &["-m", "Json.NET"]);

    project.add(Language::Swift, {
        let mut i = Instance::new("simple");
        i.args(&["-m", "simple"]);
        i
    });

    project.add(Language::Swift, {
        let mut i = Instance::new("codable");
        i.args(&["-m", "codable"]);
        i
    });

    it::entry(&mut project);

    let before = Instant::now();

    let res = project
        .runners(&root)?
        .into_par_iter()
        .filter(|s| {
            filters
                .iter()
                .all(|term| s.keywords().into_iter().any(|k| k.contains(term)))
        })
        .map(|s| s.run())
        .collect::<Vec<_>>();

    let res = res.into_iter()
        .flat_map(|res| match res {
            Ok(_) => None.into_iter(),
            Err(e) => Some(e).into_iter(),
        })
        .collect::<Vec<_>>();

    let duration = Instant::now() - before;
    println!("Finished in {}", it::DurationFmt(duration));

    if res.len() > 0 {
        for (i, e) in res.iter().enumerate() {
            eprintln!("error #{}: {}", i, e);
        }

        bail!("encountered {} error(s)", res.len());
    }

    Ok(())
}

fn main() {
    if let Err(e) = try_main() {
        eprintln!("{}", e);
        process::exit(1)
    }

    process::exit(0);
}
