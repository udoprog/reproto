extern crate reproto_backend_csharp as csharp;
extern crate reproto_backend_doc as doc;
extern crate reproto_backend_go as go;
extern crate reproto_backend_java as java;
extern crate reproto_backend_js as js;
extern crate reproto_backend_json as json;
extern crate reproto_backend_python as python;
extern crate reproto_backend_reproto as reproto;
extern crate reproto_backend_rust as rust;
extern crate reproto_backend_swift as swift;
extern crate reproto_core as core;
extern crate reproto_manifest as manifest;
extern crate reproto_repository as repository;
extern crate reproto_repository_http as repository_http;
#[macro_use]
extern crate log;
extern crate toml;
extern crate url;
#[macro_use]
extern crate serde_derive;
extern crate serde;

mod config;
mod config_env;

pub use self::config_env::ConfigEnv;
use core::errors::Result;
use core::{RelativePath, Resolver};
use manifest::{Lang, Language, Manifest};
use repository::{index_from_path, index_from_url, objects_from_path, objects_from_url, Index,
                 IndexConfig, NoIndex, NoObjects, Objects, ObjectsConfig, Paths, Repository,
                 Resolvers};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

pub const DEFAULT_INDEX: &'static str = "git+https://github.com/reproto/reproto-index";
pub const MANIFEST_NAME: &'static str = "reproto.toml";

fn load_index(base: &Path, url: &str, publishing: bool, config: IndexConfig) -> Result<Box<Index>> {
    let index_path = Path::new(url);

    if index_path.is_dir() {
        let index_path = index_path
            .canonicalize()
            .map_err(|e| format!("index: bad path: {}: {}", e, index_path.display()))?;

        return index_from_path(&index_path).map_err(Into::into);
    }

    match url::Url::parse(url) {
        Err(url::ParseError::RelativeUrlWithoutBase) => {
            let path = RelativePath::new(url).to_path(base);

            index_from_path(&path).map_err(Into::into)
        }
        Err(e) => return Err(e.into()),
        Ok(url) => index_from_url(config, &url, publishing).map_err(Into::into),
    }
}

fn load_objects(
    index: &Index,
    index_publishing: bool,
    index_url: &str,
    objects: Option<String>,
    config: ObjectsConfig,
) -> Result<Box<Objects>> {
    let (objects_url, publishing) = if let Some(ref objects) = objects {
        (objects.as_ref(), true)
    } else {
        (index.objects_url()?, index_publishing)
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
        Ok(url) => objects_from_url(
            config,
            &url,
            |config, scheme, url| match scheme {
                "http" => Ok(Some(repository_http::objects_from_url(config, url)?)),
                "https" => Ok(Some(repository_http::objects_from_url(config, url)?)),
                _ => Ok(None),
            },
            publishing,
        ).map_err(Into::into),
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
    let mut cache_home = None;
    let mut index = repository.index.clone();
    let mut objects = repository.objects.clone();

    if let Some(config_env) = ConfigEnv::new()? {
        repo_dir = Some(config_env.repo_dir);
        cache_home = Some(config_env.cache_home);
        index = index.or(config_env.index.clone());
        objects = objects.or(config_env.objects.clone());
    }

    let repo_dir = repo_dir.ok_or_else(|| "repo_dir: must be specified")?;

    // NB: do not permit publishing to default index.
    let (index_url, index_publishing) = index
        .map(|index| (index, true))
        .unwrap_or_else(|| (DEFAULT_INDEX.to_owned(), false));

    let index_config = IndexConfig {
        repo_dir: repo_dir.clone(),
    };

    let index = load_index(base, index_url.as_str(), index_publishing, index_config)?;

    let objects_config = ObjectsConfig {
        repo_dir,
        cache_home,
        missing_cache_time: Some(Duration::new(60, 0)),
    };

    let objects = load_objects(
        index.as_ref(),
        index_publishing,
        index_url.as_str(),
        objects,
        objects_config,
    )?;

    Ok(Repository::new(index, objects))
}

/// Setup the path-based resolver from a manifest.
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

/// Set up the all resolvers based on this manifest.
pub fn resolver(manifest: &Manifest) -> Result<Box<Resolver>> {
    let mut resolvers: Vec<Box<Resolver>> = Vec::new();

    resolvers.push(Box::new(repository(manifest)?));

    if let Some(resolver) = path_resolver(manifest)? {
        resolvers.push(resolver);
    }

    Ok(Box::new(Resolvers::new(resolvers)))
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
