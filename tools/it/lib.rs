extern crate diff;
#[macro_use]
extern crate failure;
extern crate relative_path;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json as json;
extern crate walkdir;

use failure::ResultExt;
use relative_path::{RelativePath, RelativePathBuf};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::result;
use std::str;
use std::time::{Duration, Instant};

pub mod utils;

#[derive(Debug)]
pub struct Test(String);

impl fmt::Display for Test {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(fmt)
    }
}

#[derive(Debug)]
pub struct JsonMismatch {
    pub index: usize,
    pub expected: json::Value,
    pub actual: json::Value,
}

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "check failed")]
    CheckFailed {
        expected: Option<CheckResult>,
        actual: CheckResult,
    },

    #[fail(display = "differences between two directories")]
    Differences {
        source: PathBuf,
        target: PathBuf,
        errors: Vec<utils::Diff>,
    },

    #[fail(display = "mismatches in json documents")]
    JsonMismatches { mismatches: Vec<JsonMismatch> },
}

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
    let res = cb().context(Test(id.to_string())).map_err(Into::into);
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
    /// Print debug output.
    debug: bool,
}

impl Reproto {
    pub fn from_project(cli: PathBuf, debug: bool) -> Result<Reproto> {
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
                return Ok(Self::new(binary, debug));
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

    pub fn new(binary: PathBuf, debug: bool) -> Self {
        Self { binary, debug }
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

        let reproto_toml = manifest.current_dir.join("reproto.toml");

        if reproto_toml.is_file() {
            cmd.args(&[
                "--manifest-path",
                reproto_toml.display().to_string().as_str(),
            ]);
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

        if self.debug {
            println!("reproto: {:?}", cmd);
        }

        let output = cmd.output()
            .map_err(|e| format_err!("bad exit status: {}", e))?;

        let stdout = str::from_utf8(&output.stdout)?;

        if self.debug && !stdout.is_empty() {
            println!("reproto (stdout): {}", stdout);
        }

        if !output.status.success() {
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

    /// Check a reproto project.
    pub fn check(&self, check: Check) -> Result<CheckResult> {
        if !check.path.is_dir() {
            bail!("No such proto path: {}", check.path.display());
        }

        let mut cmd = Command::new(&self.binary);

        if false {
            cmd.arg("--debug");
        }

        cmd.arg("check");

        let reproto_toml = check.current_dir.join("reproto.toml");

        if reproto_toml.is_file() {
            cmd.args(&[
                "--manifest-path",
                reproto_toml.display().to_string().as_str(),
            ]);
        }

        // Do not use the local repository.
        cmd.arg("--no-repository");
        // Path to resolve packages from.
        cmd.args(&["--path", check.path.display().to_string().as_str()]);

        cmd.args(&[check.package.as_str()]);

        let output = cmd.output()
            .map_err(|e| format_err!("failed to spawn reproto: {}", e))?;

        let stdout = str::from_utf8(&output.stdout)?;
        let stderr = str::from_utf8(&output.stderr)?;

        let stdout = if !stdout.is_empty() {
            stdout.lines().map(|s| s.to_string()).collect()
        } else {
            vec![]
        };

        let stderr = if !stderr.is_empty() {
            stderr.lines().map(|s| s.to_string()).collect()
        } else {
            vec![]
        };

        Ok(CheckResult {
            status: output.status.success(),
            stdout: stdout,
            stderr: stderr,
        })
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

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckResult {
    status: bool,
    stdout: Vec<String>,
    stderr: Vec<String>,
}

impl fmt::Display for CheckResult {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        writeln!(fmt, "status: {}", self.status)?;

        if self.stdout.is_empty() {
            writeln!(fmt, "stdout *empty*")?;
        } else {
            writeln!(fmt, "stdout:")?;

            for line in &self.stdout {
                writeln!(fmt, "{}", line)?;
            }
        }

        if self.stderr.is_empty() {
            writeln!(fmt, "stderr *empty*")?;
        } else {
            writeln!(fmt, "stderr:")?;

            for line in &self.stderr {
                writeln!(fmt, "{}", line)?;
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct Check<'m> {
    /// Path to build packages from.
    path: &'m Path,
    /// Working directory.
    current_dir: &'m Path,
    /// Build the given packages.
    package: &'m String,
}

#[derive(Debug)]
pub struct CheckRunner<'a> {
    /// Name of the test.
    test: &'a str,
    /// Instance of this test.
    instance: String,
    /// Package to build.
    package: String,
    /// Action to run.
    action: Action,
    /// Path to build packages from.
    path: PathBuf,
    /// Current project directory.
    current_dir: PathBuf,
    /// Reproto command wrapper.
    reproto: &'a Reproto,
}

/// Perform a check and compare expected errors.
impl<'a> CheckRunner<'a> {
    pub fn check<'m>(&'m self) -> Check<'m> {
        Check {
            path: &self.path,
            current_dir: &self.current_dir,
            package: &self.package,
        }
    }

    fn try_run(&self) -> Result<()> {
        let checks = self.current_dir.join("checks");
        let check_path = checks.join(&self.package).with_extension("json");

        match self.action {
            Action::Update => {
                if !checks.is_dir() {
                    fs::create_dir_all(&checks).map_err(|e| {
                        format_err!("failed to create directory: {}: {}", checks.display(), e)
                    })?;
                }

                let result = self.reproto.check(self.check())?;

                let bytes = json::to_string_pretty(&result)?;

                let mut f = File::create(check_path)?;
                f.write_all(bytes.as_bytes())?;
            }
            Action::Verify => {
                let actual = self.reproto.check(self.check())?;

                if !check_path.is_file() {
                    return Err(Error::CheckFailed {
                        expected: None,
                        actual: actual,
                    }.into());
                }

                let f = File::open(&check_path)
                    .map_err(|e| format_err!("failed to open: {}: {}", check_path.display(), e))?;

                let doc = json::Deserializer::from_reader(f)
                    .into_iter()
                    .next()
                    .ok_or_else(|| {
                        format_err!("Expected one document in: {}", check_path.display())
                    })?;

                let expected: CheckResult = doc?;

                if actual != expected {
                    bail!("ACTUAL:\n{}\nEXPECTED:\n{}", actual, expected);
                }
            }
        }

        Ok(())
    }
}

impl<'a> Runner for CheckRunner<'a> {
    fn keywords(&self) -> Vec<&str> {
        vec![
            "check",
            self.test,
            self.instance.as_str(),
            self.package.as_str(),
        ]
    }

    /// Run the suite.
    fn run(&self) -> Result<()> {
        let id = format!(
            "check {} (instance: {}, package: {})",
            self.test,
            self.instance.as_str(),
            self.package.as_str(),
        );

        timed_run(id, || self.try_run())
    }
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

        let mut mismatches = Vec::new();

        for (i, (actual, expected)) in actual.into_iter().zip(expected).enumerate() {
            if similar(&actual, &expected) {
                continue;
            }

            mismatches.push(JsonMismatch {
                index: i,
                actual,
                expected,
            });
        }

        if !mismatches.is_empty() {
            return Err(Error::JsonMismatches { mismatches }.into());
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
        vec![
            "project",
            self.test,
            self.instance.as_str(),
            self.language.name(),
        ]
    }

    /// Run the suite.
    fn run(&self) -> Result<()> {
        let id = format!(
            "project {} (lang: {}, instance: {})",
            self.test,
            self.language.name(),
            self.instance.as_str(),
        );

        timed_run(id, || self.try_run())
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

                return Err(Error::Differences {
                    source: self.expected_struct.to_owned(),
                    target: self.target_struct.to_owned(),
                    errors: errors,
                }.into());
            }
        }

        Ok(())
    }
}

impl<'a> Runner for StructureRunner<'a> {
    fn keywords(&self) -> Vec<&str> {
        vec![
            "structure",
            self.test,
            self.instance.as_str(),
            self.language.name(),
        ]
    }

    /// Run the suite.
    fn run(&self) -> Result<()> {
        let id = format!(
            "structure {} (lang: {}, instance: {})",
            self.test,
            self.language.name(),
            self.instance.as_str(),
        );

        timed_run(id, || self.try_run())
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
    /// If we should automatically discovery checks or not.
    discover_checks: bool,
    /// Package to run checks for.
    checks: Vec<String>,
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
            discover_checks: false,
            checks: vec![],
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

    /// Assume that the test is meant to checks, and discover all available checks.
    pub fn discover_checks(&mut self) {
        self.discover_checks = true;
    }

    /// Set the package to check.
    pub fn check<P: AsRef<str>>(&mut self, package: P) {
        self.checks.push(package.as_ref().to_string());
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
    project_languages: &'a HashSet<Language>,
    all_languages: &'a [Language],
    reproto: &'a Reproto,
    /// Run checks.
    do_checks: bool,
    /// Run the suite.
    do_structures: bool,
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
        project_languages: &'a HashSet<Language>,
        all_languages: &'a [Language],
        reproto: &'a Reproto,
        do_checks: bool,
        do_structures: bool,
        do_project: bool,
        action: Action,
    ) -> Self {
        Self {
            project_languages: project_languages,
            all_languages: all_languages,
            reproto: reproto,
            do_checks: do_checks,
            do_structures: do_structures,
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

            // structure and project entrypoint
            let entry_reproto = current_dir.join("proto").join("test.reproto");

            let path = current_dir.join("proto");
            let input = current_dir.join("input");

            let mut checks = suite.checks.clone();

            // read the path for reproto files.
            if suite.discover_checks {
                for e in fs::read_dir(&path)? {
                    let e = e?;
                    let p = e.path();

                    let has_reproto_ext = p.extension()
                        .and_then(|s| s.to_str())
                        .map(|e| e == "reproto")
                        .unwrap_or(false);

                    if p.is_file() && has_reproto_ext {
                        if let Some(file_stem) = p.file_stem().and_then(|s| s.to_str()) {
                            checks.push(file_stem.to_string());
                        } else {
                            bail!("no file stem: {}", p.display());
                        }
                    }
                }
            }

            let mut inputs = Vec::new();

            inputs.extend(json_files(&current_dir)?);
            inputs.extend(files_in_dir(&input)?);

            if self.do_checks {
                for package in &checks {
                    for instance in &default_instances {
                        runners.push(Box::new(CheckRunner {
                            test: suite.test,
                            instance: instance.name.to_string(),
                            package: package.to_string(),
                            action: self.action,
                            path: path.clone(),
                            current_dir: current_dir.clone(),
                            reproto: self.reproto,
                        }));
                    }
                }
            }

            for language in self.all_languages.iter() {
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

                    if !entry_reproto.is_file() {
                        continue;
                    }

                    if self.do_project && language.supports_project()
                        && self.project_languages.contains(language)
                    {
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

                    if self.do_structures {
                        let expected_struct =
                            language.path(root, &[suite.test, "structures", name]);
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
