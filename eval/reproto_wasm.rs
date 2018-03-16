extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
#[macro_use]
extern crate stdweb;

extern crate reproto_ast as ast;
extern crate reproto_backend_csharp as csharp;
extern crate reproto_backend_go as go;
extern crate reproto_backend_java as java;
extern crate reproto_backend_js as js;
extern crate reproto_backend_json as json;
extern crate reproto_backend_python as python;
extern crate reproto_backend_reproto as reproto;
extern crate reproto_backend_rust as rust;
extern crate reproto_backend_swift as swift;
extern crate reproto_compile as compile;
extern crate reproto_core as core;
extern crate reproto_derive as derive;
extern crate reproto_manifest as manifest;
extern crate reproto_parser as parser;

use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;
use std::str;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
enum Format {
    #[serde(rename = "json")]
    Json,
    #[serde(rename = "yaml")]
    Yaml,
    #[serde(rename = "reproto")]
    Reproto,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
enum Output {
    #[serde(rename = "reproto")]
    Reproto,
    #[serde(rename = "java")]
    Java,
    #[serde(rename = "csharp")]
    Csharp,
    #[serde(rename = "go")]
    Go,
    #[serde(rename = "swift")]
    Swift,
    #[serde(rename = "python")]
    Python,
    #[serde(rename = "rust")]
    Rust,
    #[serde(rename = "js")]
    JavaScript,
    #[serde(rename = "json")]
    Json,
}

impl Output {
    /// Convert into a manifest language and accumulate modules.
    fn into_lang(self, settings: Settings, modules: &mut Vec<Box<Any>>) -> Box<manifest::Lang> {
        match self {
            Output::Reproto => Box::new(reproto::ReprotoLang),
            Output::Java => {
                if settings.java.jackson {
                    modules.push(Box::new(java::JavaModule::Jackson));
                }

                if settings.java.lombok {
                    modules.push(Box::new(java::JavaModule::Lombok));
                }

                Box::new(java::JavaLang)
            }
            Output::Csharp => {
                if settings.csharp.json_net {
                    modules.push(Box::new(csharp::CsharpModule::JsonNet));
                }

                Box::new(csharp::CsharpLang)
            }
            Output::Go => {
                if settings.go.encoding_json {
                    modules.push(Box::new(go::GoModule::EncodingJson));
                }

                Box::new(go::GoLang)
            }
            Output::Swift => {
                if settings.swift.codable {
                    modules.push(Box::new(swift::SwiftModule::Codable));
                }

                if settings.swift.simple {
                    modules.push(Box::new(swift::SwiftModule::Simple));
                }

                Box::new(swift::SwiftLang)
            }
            Output::Python => Box::new(python::PythonLang),
            Output::Rust => {
                if settings.rust.chrono {
                    modules.push(Box::new(rust::RustModule::Chrono));
                }

                Box::new(rust::RustLang)
            }
            Output::JavaScript => Box::new(js::JsLang),
            Output::Json => Box::new(json::JsonLang),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JavaSettings {
    jackson: bool,
    lombok: bool,
}

js_serializable!(JavaSettings);
js_deserializable!(JavaSettings);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CsharpSettings {
    json_net: bool,
}

js_serializable!(CsharpSettings);
js_deserializable!(CsharpSettings);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GoSettings {
    encoding_json: bool,
}

js_serializable!(GoSettings);
js_deserializable!(GoSettings);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SwiftSettings {
    codable: bool,
    simple: bool,
}

js_serializable!(SwiftSettings);
js_deserializable!(SwiftSettings);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RustSettings {
    chrono: bool,
}

js_serializable!(RustSettings);
js_deserializable!(RustSettings);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Settings {
    java: JavaSettings,
    swift: SwiftSettings,
    rust: RustSettings,
    csharp: CsharpSettings,
    go: GoSettings,
}

js_serializable!(Settings);
js_deserializable!(Settings);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct File {
    package: String,
    version: Option<String>,
    content: String,
}

js_serializable!(File);
js_deserializable!(File);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
enum Content {
    #[serde(rename = "content")]
    Content { content: String },
    #[serde(rename = "file_index")]
    FileIndex { index: usize },
}

js_serializable!(Content);
js_deserializable!(Content);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Derive {
    content: Content,
    files: Vec<File>,
    root_name: String,
    format: Format,
    output: Output,
    package_prefix: Option<String>,
    settings: Settings,
}

js_serializable!(Derive);
js_deserializable!(Derive);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Marker {
    message: String,
    row_start: u32,
    row_end: u32,
    col_start: u32,
    col_end: u32,
}

impl Marker {
    /// Convert an error into a marker.
    fn try_from_error(p: &core::ErrorPos, message: &str) -> core::errors::Result<Marker> {
        let (_, line, (s, e)) = core::utils::find_line(p.object.read()?, (p.start, p.end))?;

        let marker = Marker {
            message: message.to_string(),
            row_start: line as u32,
            row_end: line as u32,
            col_start: s as u32,
            col_end: e as u32,
        };

        Ok(marker)
    }

    /// Safe building of markers with fallback.
    fn try_from_error_fb(p: &core::ErrorPos, message: &str) -> Marker {
        if let Ok(m) = Self::try_from_error(p, message) {
            return m;
        }

        // no positional information
        Self::safe_from_error(message)
    }

    /// Safely build a marker.
    fn safe_from_error(message: &str) -> Marker {
        Marker {
            message: message.to_string(),
            row_start: 0,
            row_end: 0,
            col_start: 0,
            col_end: 0,
        }
    }
}

js_serializable!(Marker);
js_deserializable!(Marker);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeriveFile {
    path: String,
    content: String,
}

js_serializable!(DeriveFile);
js_deserializable!(DeriveFile);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeriveResult {
    files: Vec<DeriveFile>,
    error: Option<String>,
    error_markers: Vec<Marker>,
    info_markers: Vec<Marker>,
}

js_serializable!(DeriveResult);
js_deserializable!(DeriveResult);

#[derive(Debug, Clone)]
struct ParsedFile {
    package: core::RpPackage,
    version: Option<core::Version>,
    content: String,
}

/// Resolver using provided files.
struct MapResolver(Vec<ParsedFile>);

impl core::Resolver for MapResolver {
    fn resolve(
        &mut self,
        required: &core::RpRequiredPackage,
    ) -> core::errors::Result<Vec<core::Resolved>> {
        let mut out = Vec::new();

        let package = &required.package;

        for file in self.0.iter() {
            if file.package != required.package {
                continue;
            }

            if file.version
                .as_ref()
                .map(|v| required.range.matches(v))
                .unwrap_or_else(|| required.range.matches_any())
            {
                let bytes = file.content.as_bytes().to_vec();
                let object = Box::new(core::BytesObject::new(package.to_string(), Arc::new(bytes)));

                out.push(core::Resolved {
                    version: file.version.clone(),
                    object: object,
                })
            }
        }

        Ok(out)
    }

    fn resolve_by_prefix(
        &mut self,
        prefix: &core::RpPackage,
    ) -> core::errors::Result<Vec<core::ResolvedByPrefix>> {
        let mut out = Vec::new();

        for file in self.0.iter() {
            if file.package.starts_with(prefix) {
                let bytes = file.content.as_bytes().to_vec();

                let object = Box::new(core::BytesObject::new(
                    file.package.to_string(),
                    Arc::new(bytes),
                ));

                out.push(core::ResolvedByPrefix {
                    package: file.package.clone(),
                    object: object,
                })
            }
        }

        Ok(out)
    }
}

fn derive(derive: Derive) -> DeriveResult {
    let errors = Rc::new(RefCell::new(Vec::new()));

    return match try_derive(derive, errors.clone()) {
        Ok(result) => DeriveResult {
            files: result,
            error: None,
            error_markers: vec![],
            info_markers: vec![],
        },
        Err(e) => {
            let mut error_markers = Vec::new();
            let mut info_markers = Vec::new();

            if let Some(p) = e.pos() {
                error_markers.push(Marker::try_from_error_fb(p, e.message()));
            }

            for e in errors.borrow().iter() {
                match *e {
                    core::ContextItem::ErrorPos(ref p, ref message) => {
                        error_markers.push(Marker::try_from_error_fb(p, message.as_str()));
                    }
                    core::ContextItem::InfoPos(ref p, ref message) => {
                        info_markers.push(Marker::try_from_error_fb(p, message.as_str()));
                    }
                }
            }

            DeriveResult {
                files: vec![],
                error: Some(e.display().to_string()),
                error_markers: error_markers,
                info_markers: info_markers,
            }
        }
    };

    fn try_derive(
        derive: Derive,
        errors: Rc<RefCell<Vec<core::ContextItem>>>,
    ) -> core::errors::Result<Vec<DeriveFile>> {
        let (object, package) = match derive.content {
            Content::Content { ref content } => {
                let bytes = content.as_bytes().to_vec();
                let object = core::BytesObject::new("web".to_string(), Arc::new(bytes));

                (object, None)
            }
            Content::FileIndex { index } => {
                let file = derive
                    .files
                    .get(index)
                    .ok_or_else(|| format!("No file for index: {}", index))?;

                let bytes = file.content.as_bytes().to_vec();
                let object = core::BytesObject::new(file.package.to_string(), Arc::new(bytes));

                let package = parse_package(&file)?;

                (object, Some(package))
            }
        };

        let package_prefix = derive
            .package_prefix
            .as_ref()
            .map(|s| core::RpPackage::parse(s))
            .unwrap_or_else(|| core::RpPackage::parse("io.reproto.github"));

        let input = match derive.format {
            Format::Json => derive_file(&derive, &package_prefix, &object, Box::new(derive::Json))?,
            Format::Yaml => derive_file(&derive, &package_prefix, &object, Box::new(derive::Yaml))?,
            Format::Reproto => compile::Input::Object(Box::new(object), package),
        };

        let files = parse_files(derive.files)?;

        let resolver = Box::new(MapResolver(files));

        let simple_compile = compile::SimpleCompile::new(input)
            .with_errors(errors)
            .resolver(resolver)
            .package_prefix(package_prefix);

        let mut modules = Vec::new();

        let settings = derive.settings;
        let lang = derive.output.into_lang(settings, &mut modules);

        let mut files = Vec::new();

        compile::simple_compile(
            |path, content| {
                files.push(DeriveFile {
                    path: path.display().to_string(),
                    content: content.to_string(),
                });

                Ok(())
            },
            simple_compile,
            modules,
            lang.as_ref(),
        )?;

        Ok(files)
    }

    fn parse_files(files: Vec<File>) -> core::errors::Result<Vec<ParsedFile>> {
        let mut out: Vec<ParsedFile> = Vec::new();

        for f in files {
            let package = parse_package(&f)?;

            out.push(ParsedFile {
                package: package.package,
                version: package.version,
                content: f.content,
            });
        }

        Ok(out)
    }

    fn parse_package(file: &File) -> core::errors::Result<core::RpVersionedPackage> {
        let package = core::RpPackage::parse(file.package.as_str());

        let version = if let Some(ref version) = file.version {
            Some(core::Version::parse(version.as_str())
                .map_err(|e| format!("{}: failed to parse version `{}`: {}", package, version, e))?)
        } else {
            None
        };

        Ok(core::RpVersionedPackage::new(package, version))
    }

    fn derive_file<'input>(
        derive: &Derive,
        package_prefix: &core::RpPackage,
        object: &'input core::Object,
        format: Box<derive::Format>,
    ) -> core::errors::Result<compile::Input<'input>> {
        let decl = derive::derive(
            derive::Derive::new(
                derive.root_name.to_string(),
                format,
                Some(package_prefix.clone()),
            ),
            object,
        )?;

        let file = ast::File {
            comment: vec!["Generated from reproto derive".to_string().into()],
            uses: vec![],
            attributes: vec![],
            decls: vec![decl],
        };

        let input = compile::Input::File(
            file,
            Some(core::RpVersionedPackage::new(package_prefix.clone(), None)),
        );

        Ok(input)
    }
}

fn main() {
    stdweb::initialize();

    js! {
        Module.exports.derive = @{derive};
    }
}
