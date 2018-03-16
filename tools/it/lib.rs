extern crate diff;
#[macro_use]
extern crate failure;
extern crate relative_path;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json as json;
extern crate walkdir;

use relative_path::{RelativePath, RelativePathBuf};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::result;
use std::str;
use std::time::{Duration, Instant};

mod utils;

pub type Result<T> = result::Result<T, failure::Error>;

#[macro_export]
macro_rules! define {
    ($($test:ident => $blk:block,)*) => {
        $(
        #[allow(unused)]
        fn $test($test: &mut $crate::Suite) $blk
        )*

        pub fn entry<'a>(project: &mut $crate::Project<'a>) {
            $(
            let test = stringify!($test).trim_matches('_');
            let mut suite = $crate::Suite::new(test);
            $test(&mut suite);
            project.suite(suite);
            )*
        }
    }
}

macro_rules! tests {
    ($($name:ident)*) => {
        $(
        mod $name {
            include!(concat!("../../it/", stringify!($name), ".rs"));
        }
        )*

        pub fn entry(project: &mut $crate::Project) {
            $(
            $name::entry(project);
            )*
        }
    }
}

tests!(tests);

#[derive(Debug)]
pub struct DurationFmt(pub Duration);

impl fmt::Display for DurationFmt {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let d = self.0;

        let (secs, ns) = (d.as_secs(), d.subsec_nanos());

        if secs > 0 {
            return write!(fmt, "{}.{}s", secs, (ns / 10_000_000) % 100);
        }

        match ns {
            ns if ns / 1_000_000 > 0 => write!(fmt, "{}ms", ns / 1_000_000),
            ns if ns / 1_000 > 0 => write!(fmt, "{}us", ns / 1_000),
            ns => write!(fmt, "{}ns", ns),
        }
    }
}

/// Perform a timed run of the given segment and report its results.
pub fn timed_run<C>(id: String, cb: C) -> Result<()>
where
    C: FnOnce() -> Result<()>,
{
    let before = Instant::now();
    let res = cb().map_err(|e| format_err!("{}: {}", id, e));
    let duration = Instant::now() - before;

    if res.is_err() {
        println!("FAIL {} ({})", id, DurationFmt(duration));
    } else {
        println!("  OK {} ({})", id, DurationFmt(duration));
    }

    res
}

/// Wrapping the reproto command invocation.
#[derive(Debug, Clone)]
pub struct Reproto {
    /// Path to binary.
    binary: PathBuf,
}

impl Reproto {
    pub fn from_project(cli: PathBuf) -> Result<Reproto> {
        let mut cmd = Command::new("cargo");

        cmd.arg("build");
        cmd.arg("--manifest-path");
        cmd.arg(cli.join("Cargo.toml").display().to_string());
        cmd.arg("--message-format");
        cmd.arg("json");

        let mut child = cmd.stdout(Stdio::piped())
            .spawn()
            .map_err(|e| format_err!("bad exit status: {}", e))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| format_err!("failed to get stdout"))?;

        for doc in json::Deserializer::from_reader(stdout).into_iter() {
            let doc: json::Value = doc?;

            let doc: CargoLine = match CargoLine::deserialize(doc) {
                Ok(doc) => doc,
                Err(_) => continue,
            };

            if doc.target.kind.contains(&CargoKind::Bin) {
                let binary = doc.filenames
                    .into_iter()
                    .next()
                    .ok_or_else(|| format_err!("expected one file name"))?;

                let binary = Path::new(&binary).to_path_buf();
                return Ok(Self::new(binary));
            }
        }

        bail!("could not build binary");

        #[derive(Debug, Deserialize, PartialEq, Eq)]
        pub enum CargoKind {
            #[serde(rename = "bin")]
            Bin,
            #[serde(rename = "lib")]
            Lib,
        }

        #[derive(Debug, Deserialize)]
        pub struct CargoTarget {
            kind: Vec<CargoKind>,
        }

        #[derive(Debug, Deserialize)]
        pub struct CargoLine {
            filenames: Vec<String>,
            target: CargoTarget,
        }
    }

    pub fn new(binary: PathBuf) -> Self {
        Self { binary: binary }
    }

    /// Build a reproto project.
    pub fn build(&self, manifest: Manifest) -> Result<()> {
        if !manifest.path.is_dir() {
            bail!("No such proto path: {}", manifest.path.display());
        }

        let mut cmd = Command::new(&self.binary);

        if false {
            cmd.arg("--debug");
        }

        cmd.arg("build");
        cmd.args(&["--lang", manifest.language.lang()]);

        if let Some(package_prefix) = manifest.package_prefix {
            cmd.args(&["--package-prefix", package_prefix]);
        }

        // Output directory.
        cmd.args(&["-o", manifest.output.display().to_string().as_str()]);
        // Disable using local repository.
        cmd.arg("--no-repository");
        // Path to resolve packages from.
        cmd.args(&["--path", manifest.path.display().to_string().as_str()]);

        for package in manifest.packages {
            cmd.args(&["--package", package.as_str()]);
        }

        cmd.args(manifest.extra);

        let output = cmd.current_dir(manifest.current_dir)
            .output()
            .map_err(|e| format_err!("bad exit status: {}", e))?;

        if !output.status.success() {
            let stdout = str::from_utf8(&output.stdout)?;
            let stderr = str::from_utf8(&output.stderr)?;

            bail!(
                "failed to run reproto on project: {}: {}:\nstdout: {}\nstderr: {}",
                manifest.current_dir.display(),
                output.status,
                stdout,
                stderr,
            );
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Language {
    Csharp,
    Go,
    Java,
    JavaScript,
    Json,
    Python,
    Python3,
    Reproto,
    Rust,
    Swift,
}

impl Language {
    /// Does the language support building a project?
    pub fn supports_project(&self) -> bool {
        use self::Language::*;

        match *self {
            Json | Reproto => false,
            _ => true,
        }
    }

    /// Get the name of the working directory.
    pub fn name(&self) -> &'static str {
        use self::Language::*;

        match *self {
            Csharp => "csharp",
            Go => "go",
            Java => "java",
            JavaScript => "js",
            Json => "json",
            Python => "python",
            Python3 => "python3",
            Reproto => "reproto",
            Rust => "rust",
            Swift => "swift",
        }
    }

    /// Value to `--lang` argument.
    pub fn lang(&self) -> &'static str {
        use self::Language::*;

        match *self {
            Csharp => "csharp",
            Go => "go",
            Java => "java",
            JavaScript => "js",
            Json => "json",
            Python => "python",
            Python3 => "python",
            Reproto => "reproto",
            Rust => "rust",
            Swift => "swift",
        }
    }

    /// Output directory for language.
    pub fn output(&self) -> &'static RelativePath {
        use self::Language::*;

        match *self {
            Java => RelativePath::new("target/generated-sources/reproto"),
            JavaScript => RelativePath::new("generated"),
            Python => RelativePath::new("generated"),
            Python3 => RelativePath::new("generated"),
            Rust => RelativePath::new("src"),
            Swift => RelativePath::new("Sources/Models"),
            Go => RelativePath::new("models"),
            _ => RelativePath::new("."),
        }
    }

    /// Package prefix to use
    pub fn package_prefix(&self) -> Option<&'static str> {
        use self::Language::*;

        match *self {
            Rust => Some("generated"),
            _ => None,
        }
    }

    /// Working directory source.
    pub fn source_workdir(&self, root: &Path) -> PathBuf {
        root.join("workdir").join(self.name())
    }

    /// Resolve a directory with this language at the end.
    pub fn path(&self, root: &Path, parts: &[&str]) -> PathBuf {
        let mut out = root.to_owned();

        for p in parts {
            out = out.join(p);
        }

        out.join(self.name())
    }
}

pub trait Runner: Send {
    /// Check if the current runner matches the given filters.
    fn keywords(&self) -> Vec<&str> {
        vec![]
    }

    /// Run the current runner.
    fn run(&self) -> Result<()>;
}

#[derive(Debug)]
pub struct Manifest<'m> {
    /// Path to build packages from.
    path: &'m Path,
    /// Working directory.
    current_dir: &'m Path,
    /// Output directory.
    output: PathBuf,
    /// Language-specific options.
    language: &'m Language,
    /// Build the given packages.
    packages: &'m [String],
    /// Extra arguments.
    extra: &'m [String],
    /// Package prefix to apply.
    package_prefix: Option<&'m str>,
}

#[derive(Debug)]
pub struct ProjectRunner<'a> {
    test: &'a str,
    /// Instance of this test.
    instance: String,
    /// Path to build packages from.
    path: PathBuf,
    /// Package to build.
    packages: Vec<String>,
    /// Inputs to feed to the project.
    inputs: Vec<PathBuf>,
    /// Source directory from where to build project.
    source_workdir: PathBuf,
    /// Target directory to build project.
    target_workdir: PathBuf,
    /// Current project directory.
    current_dir: PathBuf,
    /// Language-specific options.
    language: &'a Language,
    /// Reproto command wrapper.
    reproto: &'a Reproto,
    /// Extra arguments.
    extra: Vec<String>,
}

impl<'a> ProjectRunner<'a> {
    pub fn manifest<'m>(&'m self) -> Manifest<'m> {
        Manifest {
            path: &self.path,
            current_dir: &self.current_dir,
            output: self.language.output().to_path(&self.target_workdir),
            language: self.language,
            packages: &self.packages,
            extra: &self.extra,
            package_prefix: self.language.package_prefix(),
        }
    }

    fn try_run(&self) -> Result<()> {
        let script = self.target_workdir.join("script.sh");

        utils::copy_dir(&self.source_workdir, &self.target_workdir)?;

        self.reproto.build(self.manifest())?;

        // building project
        let output = Command::new("make")
            .arg(self.instance.as_str())
            .current_dir(&self.target_workdir)
            .output()
            .map_err(|e| {
                format_err!(
                    "failed to build project: {}: {}",
                    self.target_workdir.display(),
                    e
                )
            })?;

        if !output.status.success() {
            let stdout = str::from_utf8(&output.stdout)?;
            let stderr = str::from_utf8(&output.stderr)?;

            bail!(
                "failed to make project: {}: {}\nstdout: {}\nstderr: {}",
                self.target_workdir.display(),
                output.status,
                stdout,
                stderr,
            );
        }

        if !script.is_file() {
            bail!("missing script.sh entrypoint: {}", script.display());
        }

        let mut child = Command::new(script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;

        let mut errors = Vec::new();
        let mut actual: Vec<json::Value> = Vec::new();
        let mut expected: Vec<json::Value> = Vec::new();

        {
            let stdin = child.stdin.take().ok_or_else(|| format_err!("no stdin"))?;
            expected.extend(write_json_inputs(&self.inputs, stdin)?);

            let stdout = child.stdout.take().ok_or_else(|| format_err!("no stdout"))?;

            for line in BufReader::new(stdout).lines() {
                let line = line?;

                match json::from_str(&line) {
                    Ok(doc) => actual.push(doc),
                    Err(e) => errors.push(e.to_string()),
                }
            }
        }

        let status = child.wait()?;

        if !status.success() {
            bail!("Child exited with non-zero exit: {}", status);
        }

        if !errors.is_empty() {
            bail!("Got bad JSON on stdout:\n{}", errors.join("\n"),);
        }

        if actual.len() != expected.len() {
            bail!(
                "number of JSON documents ({}) do not match expected ({})",
                actual.len(),
                expected.len(),
            );
        }

        let mut errors = Vec::new();

        for (i, (actual, expected)) in actual.into_iter().zip(expected).enumerate() {
            if similar(&actual, &expected) {
                continue;
            }

            errors.push(format!(
                "#{} JSON mismatch: {} (actual) != {} (expected)",
                i, actual, expected
            ));
        }

        if !errors.is_empty() {
            bail!("test failed: {}", errors.join(", "));
        }

        return Ok(());

        /// Write inputs to the stdin of the process and collect expected documents.
        fn write_json_inputs<W>(inputs: &[PathBuf], mut stdin: W) -> Result<Vec<json::Value>>
        where
            W: io::Write,
        {
            let mut expected = Vec::new();

            for input in inputs {
                let f = File::open(&input).map_err(|e| format_err!("{}: {}", input.display(), e))?;

                for line in BufReader::new(f).lines() {
                    let line = line?;

                    // skip comments.
                    if line.starts_with('#') {
                        continue;
                    }

                    // skip empty lines
                    if line.trim() == "" {
                        continue;
                    }

                    expected.push(json::from_str(&line)?);
                    writeln!(stdin, "{}", line)?;
                }
            }

            Ok(expected)
        }

        /// Check if the two documents are similar enough to be considered equal.
        fn similar(left: &json::Value, right: &json::Value) -> bool {
            use json::Value::*;

            match (left, right) {
                (&Null, &Null) => true,
                (&Bool(ref left), &Bool(ref right)) => left == right,
                (&Number(ref left), &Number(ref right)) => match (left, right) {
                    (l, r) if l.is_u64() && r.is_u64() => {
                        l.as_u64().unwrap() == r.as_u64().unwrap()
                    }
                    (l, r) if l.is_i64() && r.is_i64() => {
                        l.as_i64().unwrap() == r.as_i64().unwrap()
                    }
                    (l, r) if l.is_f64() && r.is_f64() => {
                        let l = l.as_f64().unwrap();
                        let r = r.as_f64().unwrap();

                        (l.fract() - r.fract()).abs() < 0.0001f64
                    }
                    _ => false,
                },
                (&String(ref left), &String(ref right)) => left == right,
                (&Array(ref left), &Array(ref right)) => {
                    if left.len() != right.len() {
                        return false;
                    }

                    left.iter().zip(right.iter()).all(|(l, r)| similar(l, r))
                }
                (&Object(ref left), &Object(ref right)) => {
                    if left.len() != right.len() {
                        return false;
                    }

                    for (key, left) in left {
                        match right.get(key) {
                            None => return false,
                            Some(right) => if !similar(left, right) {
                                return false;
                            },
                        }
                    }

                    return true;
                }
                _ => false,
            }
        }
    }
}

impl<'a> Runner for ProjectRunner<'a> {
    fn keywords(&self) -> Vec<&str> {
        vec![self.test, self.instance.as_str(), self.language.name()]
    }

    /// Run the suite.
    fn run(&self) -> Result<()> {
        timed_run(
            format!(
                "project {:20} (lang: {}, instance: {})",
                self.test,
                self.language.name(),
                self.instance.as_str(),
            ),
            || self.try_run(),
        )
    }
}

/// A runner that builds a specification and compares with a known, expected structure.
#[derive(Debug)]
pub struct StructureRunner<'a> {
    test: &'a str,
    instance: String,
    /// Action to run.
    action: Action,
    /// Path to build packages from.
    path: PathBuf,
    /// Package to build.
    packages: Vec<String>,
    /// Source directory to verify structure against.
    expected_struct: PathBuf,
    /// Target directory to build structure.
    target_struct: PathBuf,
    /// Current directory of project to build.
    current_dir: PathBuf,
    /// Language-specific options.
    language: &'a Language,
    /// Reproto command wrapper.
    reproto: &'a Reproto,
    /// Extra arguments.
    extra: Vec<String>,
}

impl<'a> StructureRunner<'a> {
    pub fn manifest<'m>(&'m self, output: &'m Path) -> Manifest<'m> {
        Manifest {
            path: &self.path,
            current_dir: &self.current_dir,
            output: output.to_owned(),
            language: self.language,
            packages: &self.packages,
            extra: &self.extra,
            package_prefix: None,
        }
    }

    fn try_run(&self) -> Result<()> {
        use utils::Diff::*;

        match self.action {
            Action::Update => {
                if self.expected_struct.is_dir() {
                    // Remove existing expected directory.
                    fs::remove_dir_all(&self.expected_struct)?;
                }

                fs::create_dir_all(&self.expected_struct)?;
                self.reproto.build(self.manifest(&self.expected_struct))?;
            }
            Action::Verify => {
                if self.target_struct.is_dir() {
                    // Remove existing directory, and re-recreate it.
                    fs::remove_dir_all(&self.target_struct)?;
                }

                fs::create_dir_all(&self.target_struct)?;
                self.reproto.build(self.manifest(&self.target_struct))?;

                let mut errors = Vec::new();
                utils::diff_recursive(&self.expected_struct, &self.target_struct, &mut errors)?;

                if errors.is_empty() {
                    return Ok(());
                }

                println!(
                    "differences between {} ({}) and {} ({})",
                    self.expected_struct.display(),
                    utils::Location::Source.display(),
                    self.target_struct.display(),
                    utils::Location::Dest.display(),
                );

                for e in errors {
                    match e {
                        MissingDir(loc, dir) => {
                            println!("missing dir in {}:{}", loc.display(), dir.display());
                        }
                        ExpectedDir(loc, file) => {
                            println!("expected dir in {}:{}", loc.display(), file.display());
                        }
                        MissingFile(loc, file) => {
                            println!("missing file in {}:{}", loc.display(), file.display());
                        }
                        ExpectedFile(loc, file) => {
                            println!("expected file in {}:{}", loc.display(), file.display());
                        }
                        Mismatch(src, _, mismatch) => {
                            let added = mismatch
                                .iter()
                                .map(|m| match m {
                                    &diff::Result::Right(_) => 1,
                                    _ => 0,
                                })
                                .sum::<u32>();

                            let removed = mismatch
                                .iter()
                                .map(|m| match m {
                                    &diff::Result::Left(_) => 1,
                                    _ => 0,
                                })
                                .sum::<u32>();

                            println!("{}: +{}, -{}", src.display(), added, removed);

                            for m in mismatch {
                                match m {
                                    diff::Result::Left(l) => println!("-{}", l),
                                    diff::Result::Right(r) => println!("+{}", r),
                                    diff::Result::Both(l, _) => println!(" {}", l),
                                }
                            }
                        }
                    }
                }

                bail!(
                    "differences between {} ({}) and {} ({})",
                    self.expected_struct.display(),
                    utils::Location::Source.display(),
                    self.target_struct.display(),
                    utils::Location::Dest.display(),
                );
            }
        }

        Ok(())
    }
}

impl<'a> Runner for StructureRunner<'a> {
    fn keywords(&self) -> Vec<&str> {
        vec![self.test, self.instance.as_str(), self.language.name()]
    }

    /// Run the suite.
    fn run(&self) -> Result<()> {
        timed_run(
            format!(
                "structure {:20} (lang: {}, instance: {})",
                self.test,
                self.language.name(),
                self.instance.as_str(),
            ),
            || self.try_run(),
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Action {
    /// Update the suite.
    Update,
    /// Verify the suite.
    Verify,
}

/// A test suite.
#[derive(Debug)]
pub struct Suite<'a> {
    test: &'a str,
    /// Proto path to build.
    proto: Vec<RelativePathBuf>,
    inputs: Vec<RelativePathBuf>,
    arguments: HashMap<Language, &'a [&'a str]>,
    /// Build the given packages.
    packages: Vec<String>,
    /// Extract suite from the given directory.
    dir: Option<RelativePathBuf>,
    /// Include only the following languages.
    include: HashSet<Language>,
}

impl<'a> Suite<'a> {
    pub fn new(test: &'a str) -> Self {
        Self {
            test: test,
            proto: Vec::new(),
            inputs: Vec::new(),
            arguments: HashMap::new(),
            packages: vec!["test".to_string()],
            dir: None,
            include: HashSet::new(),
        }
    }

    /// Build only the given languages.
    pub fn include(&mut self, language: Language) {
        self.include.insert(language);
    }

    /// Extract project configuration from the given directory.
    pub fn dir<P: AsRef<RelativePath>>(&mut self, path: P) {
        self.dir = Some(path.as_ref().to_relative_path_buf());
    }

    /// Set the package to build.
    pub fn package<P: AsRef<str>>(&mut self, package: P) {
        self.packages.push(package.as_ref().to_string());
    }

    /// Associate an argument with a given language.
    pub fn arg(&mut self, lang: Language, args: &'a [&'a str]) {
        self.arguments.insert(lang, args);
    }

    /// Hook up another proto file.
    pub fn proto<P: AsRef<RelativePath>>(&mut self, path: P) {
        self.proto.push(path.as_ref().to_owned());
    }

    /// Hook up another input file.
    pub fn input<P: AsRef<RelativePath>>(&mut self, path: P) {
        self.inputs.push(path.as_ref().to_owned());
    }
}

#[derive(Debug)]
pub struct Instance {
    name: String,
    arguments: Vec<String>,
}

impl Instance {
    pub fn new<S: AsRef<str>>(name: S) -> Self {
        Self {
            name: name.as_ref().to_string(),
            arguments: Vec::new(),
        }
    }

    /// Associate arguments with this instance.
    pub fn args(&mut self, args: &[&str]) {
        self.arguments.extend(args.iter().map(|s| s.to_string()));
    }
}

/// A project, collecting all suites.
#[derive(Debug)]
pub struct Project<'a> {
    languages: &'a [Language],
    reproto: &'a Reproto,
    /// Run the suite.
    do_suite: bool,
    /// Run the project.
    do_project: bool,
    /// Action to run.
    action: Action,
    pub suites: Vec<Suite<'a>>,
    /// Arguments to apply per language.
    arguments: HashMap<Language, &'a [&'a str]>,
    /// Instances to build.
    ///
    /// By default only a single instance named `default` is used.
    instances: HashMap<Language, Vec<Instance>>,
}

impl<'a> Project<'a> {
    pub fn new(
        languages: &'a [Language],
        reproto: &'a Reproto,
        do_suite: bool,
        do_project: bool,
        action: Action,
    ) -> Self {
        Self {
            languages: languages,
            reproto: reproto,
            do_suite: do_suite,
            do_project: do_project,
            action: action,
            suites: Vec::new(),
            arguments: HashMap::new(),
            instances: HashMap::new(),
        }
    }

    /// Add the given instance.
    pub fn add(&mut self, lang: Language, instance: Instance) {
        self.instances
            .entry(lang)
            .or_insert_with(Vec::new)
            .push(instance);
    }

    /// Associate an argument with a given language.
    pub fn arg(&mut self, lang: Language, args: &'a [&'a str]) {
        self.arguments.insert(lang, args);
    }

    /// Hook up another suite.
    pub fn suite(&mut self, suite: Suite<'a>) {
        self.suites.push(suite);
    }

    pub fn runners(&self, root: &Path) -> Result<Vec<Box<'a + Runner>>> {
        let mut runners: Vec<Box<Runner>> = Vec::new();

        let default_instances = vec![Instance::new("default")];

        for suite in &self.suites {
            let current_dir = suite
                .dir
                .as_ref()
                .map(|p| p.as_ref())
                .unwrap_or_else(|| RelativePath::new(suite.test))
                .to_path(root);

            if !current_dir.is_dir() {
                bail!("expected directory: {}", current_dir.display());
            }

            let path = current_dir.join("proto");
            let input = current_dir.join("input");

            let mut inputs = Vec::new();

            inputs.extend(json_files(&current_dir)?);
            inputs.extend(files_in_dir(&input)?);

            for language in self.languages {
                if !suite.include.is_empty() && !suite.include.contains(language) {
                    continue;
                }

                let mut extra = Vec::new();

                if let Some(args) = suite.arguments.get(language) {
                    extra.extend(args.iter().map(|a| a.to_string()));
                }

                if let Some(args) = self.arguments.get(language) {
                    extra.extend(args.iter().map(|a| a.to_string()));
                }

                let instances = self.instances.get(language).unwrap_or(&default_instances);

                for instance in instances {
                    let mut extra = extra.clone();

                    if !instance.arguments.is_empty() {
                        extra.extend(instance.arguments.iter().cloned());
                    }

                    let name = &instance.name.as_str();

                    if self.do_project && language.supports_project() {
                        let source_workdir = language.source_workdir(root);
                        let target_workdir =
                            language.path(root, &["target", "workdir", suite.test, name]);

                        runners.push(Box::new(ProjectRunner {
                            test: suite.test,
                            instance: name.to_string(),
                            path: path.clone(),
                            packages: suite.packages.clone(),
                            inputs: inputs.clone(),
                            source_workdir: source_workdir,
                            target_workdir: target_workdir,
                            current_dir: current_dir.clone(),
                            language: language,
                            reproto: self.reproto,
                            extra: extra.clone(),
                        }));
                    }

                    if self.do_suite {
                        let expected_struct =
                            language.path(root, &["expected-structures", suite.test, name]);
                        let target_struct =
                            language.path(root, &["target", "structures", suite.test, name]);

                        runners.push(Box::new(StructureRunner {
                            test: suite.test,
                            instance: name.to_string(),
                            action: self.action,
                            path: path.clone(),
                            packages: suite.packages.clone(),
                            expected_struct: expected_struct,
                            target_struct: target_struct,
                            current_dir: current_dir.clone(),
                            language: language,
                            reproto: self.reproto,
                            extra: extra.clone(),
                        }));
                    }
                }
            }
        }

        return Ok(runners);

        /// Read path to all JSON files in the given directory.
        fn json_files(dir: &Path) -> Result<Vec<PathBuf>> {
            let mut out = Vec::new();

            for e in fs::read_dir(dir)? {
                let p = e?.path();

                let has_ext = p.extension().map(|ext| ext == "json").unwrap_or(false);

                if p.is_file() && has_ext {
                    out.push(p);
                }
            }

            Ok(out)
        }

        /// Read all files in directory.
        fn files_in_dir(dir: &Path) -> Result<Vec<PathBuf>> {
            let mut out = Vec::new();

            if !dir.is_dir() {
                return Ok(out);
            }

            for e in fs::read_dir(&dir)? {
                let p = e?.path();

                if p.is_file() {
                    out.push(p);
                }
            }

            Ok(out)
        }
    }
}
