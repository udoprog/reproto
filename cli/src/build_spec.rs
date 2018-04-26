use clap::ArgMatches;
use core::errors::{Error, Result, ResultExt};
use core::{Context, CoreFlavor, Diagnostics, Flavor, Resolved, ResolvedByPrefix, Resolver,
           RpChannel, RpPackage, RpPackageFormat, RpRequiredPackage, RpVersionedPackage, Source,
           Version, Import};
use env;
use manifest::{self, Lang, Language, Manifest, ManifestFile, Publish};
use repository::Repository;
use semck;
use std::fmt;
use std::fs::File;
use std::path::Path;
use std::rc::Rc;
use trans::Environment;

/// Load the manifest based on commandline arguments.
pub fn load_manifest<'a>(matches: &ArgMatches<'a>) -> Result<Manifest> {
    let mut manifest = manifest::Manifest::default();

    let path = matches
        .value_of("manifest-path")
        .map::<Result<&Path>, _>(|p| Ok(Path::new(p)))
        .unwrap_or_else(|| Ok(Path::new(env::MANIFEST_NAME)))?;

    manifest.path = Some(path.to_owned());

    if let Some(lang) = matches.value_of("lang") {
        let lang = Language::parse(lang).ok_or_else(|| format!("not a valid language: {}", lang))?;
        manifest.lang = Some(env::convert_lang(lang));
    }

    if path.is_file() {
        debug!("reading manifest: {}", path.display());
        manifest.from_yaml(File::open(&path)?, env::convert_lang)?;
    }

    matches_to_manifest(&mut manifest, matches)?;
    return Ok(manifest);

    /// Populate manifest with overrides and extensions from the command line.
    fn matches_to_manifest(manifest: &mut Manifest, matches: &ArgMatches) -> Result<()> {
        manifest.paths.extend(
            matches
                .values_of("path")
                .into_iter()
                .flat_map(|it| it)
                .map(Path::new)
                .map(ToOwned::to_owned),
        );

        if let Some(files) = matches.values_of("file") {
            for file in files {
                match file {
                    // read from stdin
                    "-" => manifest.stdin = true,
                    // read from file
                    file => {
                        manifest
                            .files
                            .push(ManifestFile::from_path(Path::new(file)));
                    }
                }
            }
        }

        // TODO: we want to be able to load modules, when we have paths.
        if let Some(path) = manifest.path.as_ref() {
            if let Some(lang) = manifest.lang.as_ref() {
                for module in matches.values_of("module").into_iter().flat_map(|it| it) {
                    let module = lang.string_spec(path, module)?;
                    manifest.modules.push(module);
                }
            }
        }

        for package in matches.values_of("package").into_iter().flat_map(|it| it) {
            let parsed = RpRequiredPackage::parse(package);

            let parsed =
                parsed.chain_err(|| format!("failed to parse --package argument: {}", package))?;

            manifest.packages.push(parsed);
        }

        if let Some(package_prefix) = matches.value_of("package-prefix").map(RpPackage::parse) {
            manifest.package_prefix = Some(package_prefix);
        }

        if let Some(id_converter) = matches.value_of("id-converter") {
            manifest.id_converter = Some(id_converter.to_string());
        }

        // override output path
        if let Some(out) = matches.value_of("out").map(Path::new) {
            manifest.output = Some(out.to_owned());
        }

        matches_to_repository(&mut manifest.repository, matches)?;
        return Ok(());

        /// Populate the repository structure from CLI arguments.
        ///
        /// CLI arguments take precedence.
        fn matches_to_repository(
            repository: &mut manifest::Repository,
            matches: &ArgMatches,
        ) -> Result<()> {
            repository.no_repository =
                repository.no_repository || matches.is_present("no-repository");

            if let Some(objects) = matches.value_of("objects").map(ToOwned::to_owned) {
                repository.objects = Some(objects);
            }

            if let Some(index) = matches.value_of("index").map(ToOwned::to_owned) {
                repository.index = Some(index);
            }

            Ok(())
        }
    }
}

pub fn environment<'a>(
    ctx: Rc<Context>,
    lang: Box<Lang>,
    manifest: &Manifest,
    resolver: &'a mut Resolver,
) -> Result<Environment<'a, CoreFlavor>> {
    environment_with_hook(ctx, lang, manifest, resolver, |_| Ok(()))
}

/// Setup environment.
pub fn environment_with_hook<'a, F: 'static>(
    ctx: Rc<Context>,
    lang: Box<Lang>,
    manifest: &Manifest,
    resolver: &'a mut Resolver,
    path_hook: F,
) -> Result<Environment<'a, CoreFlavor>>
where
    F: Fn(&Path) -> Result<()>,
{
    let package_prefix = manifest.package_prefix.clone();

    let mut env = lang.into_env(ctx, package_prefix, resolver)
        .with_path_hook(path_hook);

    let mut errors: Vec<Error> = Vec::new();

    let mut stdin = manifest.stdin;

    if manifest.files.is_empty() && manifest.packages.is_empty() && manifest.path.is_none() {
        stdin = true;
    }

    // TODO: use version and package from the provided file.
    for file in &manifest.files {
        let package = file.package
            .as_ref()
            .map(|p| RpVersionedPackage::new(p.clone(), file.version.clone()));

        if let Err(e) = env.import_path(&file.path, package) {
            errors.push(e.into());
        }
    }

    for package in manifest.packages.iter().cloned() {
        match env.import(&package) {
            Err(e) => errors.push(e.into()),
            Ok(None) => errors.push(format!("no matching package: {}", package).into()),
            _ => {}
        }
    }

    if stdin {
        debug!("Reading file to build from stdin");

        let source = Source::stdin();

        if let Err(e) = env.import_source(source, None) {
            errors.push(e.into());
        }
    }

    if let Err(e) = env.verify() {
        errors.push(e.into());
    }

    if !errors.is_empty() {
        return Err(Error::new("error when building").with_suppressed(errors));
    }

    Ok(env)
}

/// Argument match.
pub struct Match(pub Version, pub Source, pub RpPackage);

/// Setup matches from a publish manifest.
pub fn publish_matches<'a, I>(
    resolver: &mut Resolver,
    version_override: Option<&Version>,
    publish: I,
) -> Result<Vec<Match>>
where
    I: IntoIterator<Item = &'a Publish>,
{
    let mut results = Vec::new();

    for publish in publish.into_iter() {
        let resolved = resolver.resolve_by_prefix(&publish.package)?;

        if resolved.is_empty() {
            return Err(format!("no matching packages found for: {}", publish.package).into());
        }

        for ResolvedByPrefix { package, source } in resolved {
            let version = version_override.unwrap_or(&publish.version).clone();
            results.push(Match(version, source, package.clone()));
        }
    }

    Ok(results)
}

pub fn matches<'a, I>(
    resolver: &mut Resolver,
    version_override: Option<&Version>,
    packages: I,
) -> Result<Vec<Match>>
where
    I: IntoIterator<Item = &'a RpRequiredPackage>,
{
    let mut results = Vec::new();

    for package in packages.into_iter() {
        let mut resolved = resolver.resolve(package)?.into_iter();

        let first = resolved
            .next()
            .ok_or_else(|| format!("no package matching: {}", package))?;

        if let Some(next) = resolved.next() {
            warn!("matched: {}", first);
            warn!("    and: {}", next);

            while let Some(next) = resolved.next() {
                warn!("    and: {}", next);
            }

            return Err("more than one matching package found".into());
        }

        let Resolved { version, source } = first;

        let version = version_override
            .cloned()
            .or(version)
            .ok_or_else(|| format!("no version for package: {}", package.package))?;

        results.push(Match(version, source, package.package.clone()));
    }

    Ok(results)
}

pub fn semck_check(
    ctx: &Context,
    errors: &mut Vec<Error>,
    repository: &mut Repository,
    env: &mut Environment<CoreFlavor>,
    m: &Match,
) -> Result<()> {
    let Match(ref version, ref next, ref package) = *m;

    // perform semck verification
    if let Some(d) = repository
        .all(package)?
        .into_iter()
        .filter(|d| d.version <= *version && !d.version.is_prerelease())
        .last()
    {
        debug!("Checking semantics of {} -> {}", d.version, version);

        let current = repository
            .get_object(&d)?
            .ok_or_else(|| format!("No object found for deployment: {:?}", d))?;

        let name = RpPackageFormat(package, Some(&d.version)).to_string();
        let current = current.with_name(name);

        let package_from = RpVersionedPackage::new(package.clone(), Some(d.version.clone()));
        let file_from = env.load_source(current.clone(), &package_from)?;

        let package_to = RpVersionedPackage::new(package.clone(), Some(version.clone()));
        let file_to = env.load_source(next.clone(), &package_to)?;

        let violations = semck::check((&d.version, &file_from), (&version, &file_to))?;

        if !violations.is_empty() {
            errors.push(Error::new(format!(
                "Encountered {} semck violation(s)",
                violations.len()
            )));

            let mut current = Diagnostics::new(current);
            let mut next = Diagnostics::new(next.clone());

            for v in violations {
                handle_violation(&mut current, &mut next, v)?;
            }

            ctx.diagnostics(current)?;
            ctx.diagnostics(next)?;
        }
    }

    return Ok(());

    fn handle_violation(
        current: &mut Diagnostics,
        next: &mut Diagnostics,
        violation: semck::Violation,
    ) -> Result<()> {
        use semck::Violation::*;

        match violation {
            DeclRemoved(c, reg) => {
                current.err(reg, format!("{}: declaration removed", c.describe()));
            }
            DeclAdded(c, reg) => {
                next.err(reg, format!("{}: declaration added", c.describe()));
            }
            RemoveField(c, field) => {
                current.err(field, format!("{}: field removed", c.describe()));
            }
            RemoveVariant(c, field) => {
                current.err(field, format!("{}: variant removed", c.describe()));
            }
            AddField(c, field) => {
                next.err(field, format!("{}: field added", c.describe()));
            }
            AddVariant(c, field) => {
                next.err(field, format!("{}: variant added", c.describe()));
            }
            FieldTypeChange(c, from_type, from, to_type, to) => {
                next.err(
                    to,
                    format!("{}: type changed to `{}`", c.describe(), to_type),
                );
                current.info(from, format!("from `{}`", from_type));
            }
            FieldNameChange(c, from_name, from, to_name, to) => {
                next.err(
                    to,
                    format!("{}: name changed to `{}`", c.describe(), to_name),
                );
                current.info(from, format!("from `{}`", from_name));
            }
            VariantOrdinalChange(c, from_ordinal, from, to_ordinal, to) => {
                next.err(
                    to,
                    format!("{}: ordinal changed to `{}`", c.describe(), to_ordinal),
                );
                current.info(from, format!("from `{}`", from_ordinal));
            }
            FieldRequiredChange(c, from, to) => {
                next.err(
                    to,
                    format!("{}: field changed to be required`", c.describe(),),
                );
                current.info(from, "from here");
            }
            AddRequiredField(c, field) => {
                next.err(field, format!("{}: required field added", c.describe()));
            }
            FieldModifierChange(c, from, to) => {
                next.err(to, format!("{}: field modifier changed", c.describe()));
                current.info(from, "from here");
            }
            AddEndpoint(c, span) => {
                next.err(span, format!("{}: endpoint added", c.describe()));
            }
            RemoveEndpoint(c, span) => {
                current.err(span, format!("{}: endpoint removed", c.describe()));
            }
            EndpointRequestChange(c, from_channel, from, to_channel, to) => {
                next.err(
                    to,
                    format!(
                        "{}: request type changed to `{}`",
                        c.describe(),
                        FmtChannel(to_channel.as_ref())
                    ),
                );
                current.info(
                    from,
                    format!("from `{}`", FmtChannel(from_channel.as_ref())),
                );
            }
            EndpointResponseChange(c, from_channel, from, to_channel, to) => {
                next.err(
                    to,
                    format!(
                        "{}: response type changed to `{}`",
                        c.describe(),
                        FmtChannel(to_channel.as_ref())
                    ),
                );
                current.err(
                    from,
                    format!("from `{}`", FmtChannel(from_channel.as_ref())),
                );
            }
        }

        return Ok(());

        /// Helper struct to display information on channels.
        struct FmtChannel<'a, F: 'static>(Option<&'a RpChannel<F>>)
        where
            F: Flavor;

        impl<'a, F: 'static> fmt::Display for FmtChannel<'a, F>
        where
            F: Flavor,
        {
            fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                match self.0 {
                    None => write!(fmt, "*empty*"),
                    Some(channel) => write!(fmt, "{}", channel),
                }
            }
        }
    }
}

/// Setup a basic environment falling back to `NoLang` unless one is specified.
pub fn simple_config<'a>(
    ctx: &Rc<Context>,
    manifest: &Manifest,
    resolver: &'a mut Resolver,
) -> Result<Environment<'a, CoreFlavor>> {
    let lang = manifest.lang_or_nolang();
    let env = environment(ctx.clone(), lang, &manifest, resolver)?;
    Ok(env)
}
