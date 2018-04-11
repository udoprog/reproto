use clap::ArgMatches;
use config_env::ConfigEnv;
use core::errors::*;
use core::{BytesObject, Context, CoreFlavor, Flavor, Object, RelativePath, Resolved,
           ResolvedByPrefix, Resolver, RpChannel, RpPackage, RpPackageFormat, RpRequiredPackage,
           RpVersionedPackage, Version};
use manifest::{self as m, read_manifest, read_manifest_preamble, Lang, Language, Manifest,
               ManifestFile, ManifestPreamble, NoLang, Publish};
use repository::{index_from_path, index_from_url, objects_from_path, objects_from_url, Index,
                 IndexConfig, NoIndex, NoObjects, Objects, ObjectsConfig, Paths, Repository,
                 Resolvers};
use repository_http;
use semck;
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use trans::Environment;
use url;

pub const DEFAULT_INDEX: &'static str = "git+https://github.com/reproto/reproto-index";
pub const MANIFEST_NAME: &'static str = "reproto.toml";

fn load_index(base: &Path, index_url: &str, config: IndexConfig) -> Result<Box<Index>> {
    let index_path = Path::new(index_url);

    if index_path.is_dir() {
        let index_path = index_path
            .canonicalize()
            .map_err(|e| format!("index: bad path: {}: {}", e, index_path.display()))?;

        return index_from_path(&index_path)
            .map(|i| Box::new(i) as Box<Index>)
            .map_err(Into::into);
    }

    match url::Url::parse(index_url) {
        Err(url::ParseError::RelativeUrlWithoutBase) => {
            let path = RelativePath::new(index_url).to_path(base);

            index_from_path(&path)
                .map(|i| Box::new(i) as Box<Index>)
                .map_err(Into::into)
        }
        Err(e) => return Err(e.into()),
        Ok(url) => index_from_url(config, &url).map_err(Into::into),
    }
}

fn load_objects(
    index: &Index,
    index_url: &str,
    objects: Option<String>,
    config: ObjectsConfig,
) -> Result<Box<Objects>> {
    let objects_url = if let Some(ref objects) = objects {
        objects.as_ref()
    } else {
        index.objects_url()?
    };

    debug!("index: {}", index_url);
    debug!("objects: {}", objects_url);

    let objects_path = Path::new(objects_url);

    if objects_path.is_dir() {
        let objects_path = objects_path
            .canonicalize()
            .map_err(|e| format!("objects: bad path: {}: {}", e, objects_path.display()))?;

        return objects_from_path(objects_path)
            .map(|o| Box::new(o) as Box<Objects>)
            .map_err(Into::into);
    }

    match url::Url::parse(objects_url) {
        // Relative to index index repository!
        Err(url::ParseError::RelativeUrlWithoutBase) => index
            .objects_from_index(RelativePath::new(objects_url))
            .map_err(Into::into),
        Err(e) => return Err(e.into()),
        Ok(url) => objects_from_url(config, &url, |config, scheme, url| match scheme {
            "http" => Ok(Some(repository_http::objects_from_url(config, url)?)),
            "https" => Ok(Some(repository_http::objects_from_url(config, url)?)),
            _ => Ok(None),
        }).map_err(Into::into),
    }
}

pub fn repository(manifest: &Manifest) -> Result<Repository> {
    let repository = &manifest.repository;

    if repository.no_repository {
        return Ok(Repository::new(Box::new(NoIndex), Box::new(NoObjects)));
    }

    let base = manifest
        .path
        .as_ref()
        .and_then(|p| p.parent())
        .ok_or_else(|| "no parent path to manifest")?;

    let mut repo_dir = None;
    let mut cache_dir = None;
    let mut index = repository.index.clone();
    let mut objects = repository.objects.clone();

    if let Some(config_env) = ConfigEnv::new()? {
        repo_dir = Some(config_env.repo_dir);
        cache_dir = Some(config_env.cache_dir);
        index = index.or(config_env.index.clone());
        objects = objects.or(config_env.objects.clone());
    }

    let repo_dir = repo_dir.ok_or_else(|| "repo_dir: must be specified")?;

    let index_url = index.unwrap_or_else(|| DEFAULT_INDEX.to_owned());
    let index_config = IndexConfig {
        repo_dir: repo_dir.clone(),
    };

    let index = load_index(base, index_url.as_str(), index_config)?;

    let objects_config = ObjectsConfig {
        repo_dir: repo_dir,
        cache_dir: cache_dir,
        missing_cache_time: Some(Duration::new(60, 0)),
    };

    let objects = load_objects(index.as_ref(), index_url.as_str(), objects, objects_config)?;
    Ok(Repository::new(index, objects))
}

pub fn path_resolver(manifest: &Manifest) -> Result<Option<Box<Resolver>>> {
    if manifest.paths.is_empty() {
        return Ok(None);
    }

    let mut published = HashMap::new();

    for publish in &manifest.publish {
        published.insert(publish.package.clone(), publish.version.clone());
    }

    Ok(Some(Box::new(Paths::new(
        manifest.paths.clone(),
        published,
    ))))
}

pub fn resolvers(manifest: &Manifest) -> Result<Box<Resolver>> {
    let mut resolvers: Vec<Box<Resolver>> = Vec::new();

    resolvers.push(Box::new(repository(manifest)?));

    if let Some(resolver) = path_resolver(manifest)? {
        resolvers.push(resolver);
    }

    Ok(Box::new(Resolvers::new(resolvers)))
}

/// Read the first part of the manifest, to determine the language used.
pub fn manifest_preamble<'a>(matches: &ArgMatches<'a>) -> Result<ManifestPreamble> {
    let manifest_path = matches
        .value_of("manifest-path")
        .map::<Result<&Path>, _>(|p| Ok(Path::new(p)))
        .unwrap_or_else(|| Ok(Path::new(MANIFEST_NAME)))?;

    if !manifest_path.is_file() {
        return Ok(ManifestPreamble::new(None, Some(manifest_path)));
    }

    debug!("reading manifest: {}", manifest_path.display());
    let reader = File::open(manifest_path.clone())?;

    read_manifest_preamble(&manifest_path, reader)
        .map_err(|e| format!("{}: {}", manifest_path.display(), e.display()).into())
}

/// Read the manifest based on the current environment.
pub fn manifest<'a>(
    lang: &Lang,
    matches: &ArgMatches<'a>,
    preamble: ManifestPreamble,
) -> Result<Manifest> {
    let path = preamble.path.clone();

    let mut manifest = read_manifest(lang, preamble).map_err(|e| {
        if let Some(path) = path {
            format!("{}: {}", path.display(), e.display()).into()
        } else {
            e
        }
    })?;

    manifest_from_matches(lang, &mut manifest, matches)?;
    Ok(manifest)
}

pub fn environment(
    lang: &Lang,
    ctx: Rc<Context>,
    manifest: &Manifest,
) -> Result<Environment<CoreFlavor>> {
    environment_with_hook(lang, ctx, manifest, |_| Ok(()))
}

/// Setup environment.
pub fn environment_with_hook<F: 'static>(
    lang: &Lang,
    ctx: Rc<Context>,
    manifest: &Manifest,
    path_hook: F,
) -> Result<Environment<CoreFlavor>>
where
    F: Fn(&Path) -> Result<()>,
{
    // manifest path, if present.
    if let Some(p) = manifest.path.as_ref() {
        path_hook(p)?;
    }

    let resolvers = resolvers(manifest)?;
    let package_prefix = manifest.package_prefix.clone();

    let mut env = lang.into_env(ctx, package_prefix, resolvers)
        .with_path_hook(path_hook);

    let mut errors = Vec::new();

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

        let mut buffer = Vec::new();

        let stdin = io::stdin();

        stdin
            .lock()
            .read_to_end(&mut buffer)
            .map_err(|e| format!("failed to read <stdin>: {}", e))?;

        let object = BytesObject::new("<stdin>".to_string(), Arc::new(buffer));

        if let Err(e) = env.import_object(&object, None) {
            errors.push(e.into());
        }
    }

    if let Err(e) = env.verify() {
        errors.push(e.into());
    }

    if !errors.is_empty() {
        return Err(Error::new("Error when building").with_suppressed(errors));
    }

    Ok(env)
}

/// Populate the repository structure from CLI arguments.
///
/// CLI arguments take precedence.
fn repository_from_matches(repository: &mut m::Repository, matches: &ArgMatches) -> Result<()> {
    repository.no_repository = repository.no_repository || matches.is_present("no-repository");

    if let Some(objects) = matches.value_of("objects").map(ToOwned::to_owned) {
        repository.objects = Some(objects);
    }

    if let Some(index) = matches.value_of("index").map(ToOwned::to_owned) {
        repository.index = Some(index);
    }

    Ok(())
}

fn manifest_from_matches(lang: &Lang, manifest: &mut Manifest, matches: &ArgMatches) -> Result<()> {
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
        for module in matches.values_of("module").into_iter().flat_map(|it| it) {
            manifest.modules.push(lang.string_spec(path, module)?);
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

    repository_from_matches(&mut manifest.repository, matches)?;
    Ok(())
}

/// Argument match.
pub struct Match(pub Version, pub Box<Object>, pub RpPackage);

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

        for ResolvedByPrefix { package, object } in resolved {
            let version = version_override.unwrap_or(&publish.version).clone();
            results.push(Match(version, object, package.clone()));
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
        let resolved = resolver.resolve(package)?;

        if resolved.is_empty() {
            return Err(format!("no matching packages found for: {}", package).into());
        }

        let mut it = resolved.into_iter();

        let first = it.next().ok_or_else(|| format!("no packages to publish"))?;

        if let Some(next) = it.next() {
            warn!("matched: {}", first);
            warn!("    and: {}", next);

            while let Some(next) = it.next() {
                warn!("    and: {}", next);
            }

            return Err("more than one matching package found".into());
        }

        let Resolved { version, object } = first;
        let version = version_override.cloned().or(version);

        let version =
            version.ok_or_else(|| format!("No version for package: {}", package.package))?;

        results.push(Match(version, object, package.package.clone()));
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
    let Match(ref version, ref object, ref package) = *m;

    // perform semck verification
    if let Some(d) = repository
        .all(package)?
        .into_iter()
        .filter(|d| d.version <= *version && !d.version.is_prerelease())
        .last()
    {
        debug!("Checking semantics of {} -> {}", d.version, version);

        let previous = repository
            .get_object(&d)?
            .ok_or_else(|| format!("No object found for deployment: {:?}", d))?;

        let name = RpPackageFormat(package, Some(&d.version)).to_string();
        let previous = previous.with_name(name);

        let package_from = RpVersionedPackage::new(package.clone(), Some(d.version.clone()));
        let file_from = env.load_object(previous.as_ref(), &package_from)?;

        let package_to = RpVersionedPackage::new(package.clone(), Some(version.clone()));
        let file_to = env.load_object(object.as_ref(), &package_to)?;

        let violations = semck::check((&d.version, &file_from), (&version, &file_to))?;

        if !violations.is_empty() {
            errors.push(Error::new(format!(
                "Encountered {} semck violation(s)",
                violations.len()
            )));

            for v in violations {
                handle_violation(ctx, v)?;
            }
        }
    }

    return Ok(());

    fn handle_violation(ctx: &Context, violation: semck::Violation) -> Result<()> {
        use semck::Violation::*;

        match violation {
            DeclRemoved(c, reg) => {
                ctx.report()
                    .err(reg, format!("{}: declaration removed", c.describe()))
                    .close();
            }
            DeclAdded(c, reg) => {
                ctx.report()
                    .err(reg, format!("{}: declaration added", c.describe()))
                    .close();
            }
            RemoveField(c, field) => {
                ctx.report()
                    .err(field, format!("{}: field removed", c.describe()))
                    .close();
            }
            RemoveVariant(c, field) => {
                ctx.report()
                    .err(field, format!("{}: variant removed", c.describe()))
                    .close();
            }
            AddField(c, field) => {
                ctx.report()
                    .err(field, format!("{}: field added", c.describe()))
                    .close();
            }
            AddVariant(c, field) => {
                ctx.report()
                    .err(field, format!("{}: variant added", c.describe()))
                    .close();
            }
            FieldTypeChange(c, from_type, from, to_type, to) => {
                ctx.report()
                    .err(
                        to,
                        format!("{}: type changed to `{}`", c.describe(), to_type),
                    )
                    .err(from, format!("from `{}`", from_type))
                    .close();
            }
            FieldNameChange(c, from_name, from, to_name, to) => {
                ctx.report()
                    .err(
                        to,
                        format!("{}: name changed to `{}`", c.describe(), to_name),
                    )
                    .err(from, format!("from `{}`", from_name))
                    .close();
            }
            VariantOrdinalChange(c, from_ordinal, from, to_ordinal, to) => {
                ctx.report()
                    .err(
                        to,
                        format!("{}: ordinal changed to `{}`", c.describe(), to_ordinal),
                    )
                    .err(from, format!("from `{}`", from_ordinal))
                    .close();
            }
            FieldRequiredChange(c, from, to) => {
                ctx.report()
                    .err(
                        to,
                        format!("{}: field changed to be required`", c.describe(),),
                    )
                    .err(from, "from here")
                    .close();
            }
            AddRequiredField(c, field) => {
                ctx.report()
                    .err(field, format!("{}: required field added", c.describe(),))
                    .close();
            }
            FieldModifierChange(c, from, to) => {
                ctx.report()
                    .err(to, format!("{}: field modifier changed", c.describe(),))
                    .err(from, "from here")
                    .close();
            }
            AddEndpoint(c, pos) => {
                ctx.report()
                    .err(pos, format!("{}: endpoint added", c.describe()))
                    .close();
            }
            RemoveEndpoint(c, pos) => {
                ctx.report()
                    .err(pos, format!("{}: endpoint removed", c.describe()))
                    .close();
            }
            EndpointRequestChange(c, from_channel, from, to_channel, to) => {
                ctx.report()
                    .err(
                        to,
                        format!(
                            "{}: request type changed to `{}`",
                            c.describe(),
                            FmtChannel(to_channel.as_ref())
                        ),
                    )
                    .err(
                        from,
                        format!("from `{}`", FmtChannel(from_channel.as_ref())),
                    )
                    .close();
            }
            EndpointResponseChange(c, from_channel, from, to_channel, to) => {
                ctx.report()
                    .err(
                        to,
                        format!(
                            "{}: response type changed to `{}`",
                            c.describe(),
                            FmtChannel(to_channel.as_ref())
                        ),
                    )
                    .err(
                        from,
                        format!("from `{}`", FmtChannel(from_channel.as_ref())),
                    )
                    .close();
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

/// Convert the manifest language to an actual language implementation.
pub fn convert_lang(input: Language) -> Box<Lang> {
    use self::Language::*;

    match input {
        Csharp => Box::new(::csharp::CsharpLang),
        Go => Box::new(::go::GoLang),
        Java => Box::new(::java::JavaLang),
        Js => Box::new(::js::JsLang),
        Json => Box::new(::json::JsonLang),
        Python => Box::new(::python::PythonLang),
        Reproto => Box::new(::reproto::ReprotoLang),
        Rust => Box::new(::rust::RustLang),
        Swift => Box::new(::swift::SwiftLang),
    }
}

/// Setup a basic environment falling back to `NoLang` unless one is specified.
pub fn simple_config(
    ctx: &Rc<Context>,
    matches: &ArgMatches,
) -> Result<(Manifest, Environment<CoreFlavor>)> {
    let preamble = manifest_preamble(matches)?;

    let lang = preamble
        .language
        .map(|l| convert_lang(l))
        .unwrap_or_else(|| Box::new(NoLang) as Box<Lang>);

    let manifest = manifest(lang.as_ref(), matches, preamble)?;

    let env = environment(lang.as_ref(), ctx.clone(), &manifest)?;

    Ok((manifest, env))
}
