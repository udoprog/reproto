use backend::*;
use backend::collecting::Collecting;
use backend::errors::*;
use backend::package_processor::PackageProcessor;
use core::*;
use pulldown_cmark as markdown;
use std::collections::HashMap;
use std::fmt::Write as FmtWrite;
use std::fs;
use std::io::Write as IoWrite;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

pub struct ProcessorOptions {
}

impl ProcessorOptions {
    pub fn new() -> ProcessorOptions {
        ProcessorOptions {}
    }
}

pub trait Listeners {
    fn configure(&self, _processor: &mut ProcessorOptions) -> Result<()> {
        Ok(())
    }
}

/// A vector of listeners is a valid listener.
impl Listeners for Vec<Box<Listeners>> {
    fn configure(&self, processor: &mut ProcessorOptions) -> Result<()> {
        for listeners in self {
            listeners.configure(processor)?;
        }

        Ok(())
    }
}

pub struct Processor {
    env: Environment,
    out_path: PathBuf,
    package_prefix: Option<RpPackage>,
    theme: String,
    listeners: Box<Listeners>,
    themes: HashMap<&'static str, &'static [u8]>,
}

const EXT: &str = "html";
const INDEX: &str = "index";

const NORMALIZE_CSS_NAME: &str = "normalize.css";
const NORMALIZE_CSS: &[u8] = include_bytes!("static/normalize.css");

const DOC_CSS_NAME: &str = "doc.css";

include!(concat!(env!("OUT_DIR"), "/themes.rs"));

fn build_themes() -> HashMap<&'static str, &'static [u8]> {
    let mut m = HashMap::new();

    for (key, value) in build_themes_vec() {
        m.insert(key, value);
    }

    m
}

impl Processor {
    pub fn new(_options: ProcessorOptions,
               env: Environment,
               out_path: PathBuf,
               package_prefix: Option<RpPackage>,
               theme: String,
               listeners: Box<Listeners>)
               -> Processor {
        Processor {
            env: env,
            out_path: out_path,
            package_prefix: package_prefix,
            theme: theme,
            listeners: listeners,
            themes: build_themes(),
        }
    }

    fn type_url(&self, type_id: &RpTypeId) -> Result<String> {
        let package = &type_id.package;
        let name = &type_id.name;

        if let Some(ref used) = name.prefix {
            let package = self.env.lookup_used(package, used)?;
            let package = self.package(package);
            let package = package.parts.join("_");
            let fragment = name.parts.join("_");
            return Ok(format!("{}.html#{}", package, fragment));
        }

        let fragment = name.parts.join("_");
        return Ok(format!("#{}", fragment));
    }

    fn markdown(input: &str) -> String {
        let p = markdown::Parser::new(input);
        let mut s = String::new();
        markdown::html::push_html(&mut s, p);
        s
    }

    fn package_file(&self, package: RpPackage) -> String {
        package.parts.join("_")
    }

    fn write_description(&self, out: &mut FmtWrite, comment: &Vec<String>) -> Result<()> {
        if comment.is_empty() {
            write!(out,
                   "<div class=\"description\"><em>no description</em></div>")?;
        } else {
            let comment = comment.join("\n");

            write!(out,
                   "<div class=\"description\">{}</div>",
                   Self::markdown(&comment))?;
        }

        Ok(())
    }

    fn write_variants<'a, I>(&self, out: &mut FmtWrite, variants: I) -> Result<()>
        where I: Iterator<Item = &'a RpLoc<Rc<RpEnumVariant>>>
    {
        write!(out, "<div class=\"variants\">")?;

        for variant in variants {
            write!(out, "<div class=\"variant\">")?;
            write!(out, "<h4 class=\"name\">{}</h4>", variant.name)?;

            self.write_description(out, &variant.comment)?;

            write!(out, "</div>")?;
        }

        write!(out, "</div>")?;

        Ok(())
    }

    fn write_type(&self, out: &mut FmtWrite, type_id: &RpTypeId, ty: &RpType) -> Result<()> {
        write!(out, "<span class=\"ty\">")?;

        match *ty {
            RpType::Double => {
                write!(out, "<span class=\"ty-double\">double</span>")?;
            }
            RpType::Float => {
                write!(out, "<span class=\"ty-float\">float</span>")?;
            }
            RpType::Signed(ref size) => {
                if let Some(ref size) = *size {
                    write!(out, "<span class=\"ty-signed\">signed/{}</span>", size)?;
                } else {
                    write!(out, "<span class=\"ty-signed\">signed</span>")?;
                }
            }
            RpType::Unsigned(ref size) => {
                if let Some(ref size) = *size {
                    write!(out, "<span class=\"ty-unsigned\">unsigned/{}</span>", size)?;
                } else {
                    write!(out, "<span class=\"ty-unsigned\">unsigned</span>")?;
                }
            }
            RpType::Boolean => {
                write!(out, "<span class=\"ty-boolean\">boolean</span>")?;
            }
            RpType::String => {
                write!(out, "<span class=\"ty-string\">string</span>")?;
            }
            RpType::Bytes => {
                write!(out, "<span class=\"ty-bytes\">bytes</span>")?;
            }
            RpType::Any => {
                write!(out, "<span class=\"ty-any\">any</span>")?;
            }
            RpType::Name(ref name) => {
                let url = self.type_url(&type_id.with_name(name.clone()))?;
                let name = name.parts.join(".");

                write!(out, "<span class=\"ty-name\">")?;
                write!(out, "<a href=\"{url}\">{name}</a>", url = url, name = name)?;
                write!(out, "</span>")?;
            }
            RpType::Array(ref inner) => {
                write!(out, "<span class=\"ty-array\">")?;
                write!(out, "<span class=\"ty-array-left\">[</span>")?;
                self.write_type(out, type_id, inner)?;
                write!(out, "<span class=\"ty-array-right\">]</span>")?;
                write!(out, "</span>")?;
            }
            RpType::Map(ref key, ref value) => {
                write!(out, "<span class=\"ty-map\">")?;
                write!(out, "<span class=\"ty-map-key\">{{</span>")?;
                self.write_type(out, type_id, key)?;
                write!(out, "<span class=\"ty-map-separator\">:</span>")?;
                self.write_type(out, type_id, value)?;
                write!(out, "<span class=\"ty-map-value\">}}</span>")?;
                write!(out, "</span>")?;
            }
        }

        write!(out, "</span>")?;
        Ok(())
    }

    fn write_fields<'a, I>(&self, out: &mut FmtWrite, type_id: &RpTypeId, fields: I) -> Result<()>
        where I: Iterator<Item = &'a RpLoc<RpField>>
    {
        write!(out, "<div class=\"fields\">")?;

        for field in fields {
            write!(out, "<div class=\"field\">")?;

            let mut name = format!("<span>{}</span>", field.name);
            let mut class = "name".to_owned();

            if field.is_optional() {
                class = format!("{} optional", class);
                name = format!("{}<span class=\"modifier\">?:</span>", name);
            } else {
                name = format!("{}<span class=\"modifier\">:</span>", name);
            };

            write!(out, "<div class=\"{class}\">", class = class)?;
            write!(out, "{name}", name = name)?;
            self.write_type(out, type_id, &field.ty)?;
            write!(out, "</div>")?;

            self.write_description(out, &field.comment)?;

            write!(out, "</div>")?;
        }

        write!(out, "</div>")?;

        Ok(())
    }

    fn write_doc<Body>(&self, out: &mut FmtWrite, body: Body) -> Result<()>
        where Body: FnOnce(&mut FmtWrite) -> Result<()>
    {
        write!(out, "<html>")?;
        write!(out, "<head>")?;

        write!(out,
               "<link rel=\"stylesheet\" type=\"text/css\" href=\"{normalize_css}\">",
               normalize_css = NORMALIZE_CSS_NAME)?;

        write!(out,
               "<link rel=\"stylesheet\" type=\"text/css\" href=\"{doc_css}\">",
               doc_css = DOC_CSS_NAME)?;

        write!(out, "</head>")?;
        write!(out, "<body>")?;

        body(out)?;

        write!(out, "</body>")?;
        write!(out, "</html>")?;

        Ok(())
    }

    fn write_stylesheets(&self) -> Result<()> {
        if !self.out_path.is_dir() {
            debug!("+dir: {}", self.out_path.display());
            fs::create_dir_all(&self.out_path)?;
        }

        let normalize_css = self.out_path.join(NORMALIZE_CSS_NAME);

        debug!("+css: {}", normalize_css.display());
        let mut f = fs::File::create(normalize_css)?;
        f.write_all(NORMALIZE_CSS)?;

        let doc_css = self.out_path.join(DOC_CSS_NAME);

        let content = self.themes.get(self.theme.as_str());

        if let Some(content) = content {
            debug!("+css: {}", doc_css.display());
            let mut f = fs::File::create(doc_css)?;
            f.write_all(content)?;
        } else {
            return Err(format!("no such theme: {}", self.theme).into());
        }

        Ok(())
    }

    fn write_index<'a, I>(&self, packages: I) -> Result<()>
        where I: Iterator<Item = &'a RpPackage>
    {
        let mut out = String::new();

        self.write_doc(&mut out, move |out| {
                write!(out, "<ul>")?;

                for package in packages {
                    let name = self.package(&package).parts.join(".");

                    let url = format!("{}.{}",
                                      self.package_file(self.package(&package)),
                                      self.ext());

                    write!(out,
                           "<li><a href=\"{url}\">{name}</a></li>",
                           url = url,
                           name = name)?;
                }

                write!(out, "</ul>")?;

                Ok(())
            })?;

        let mut path = self.out_path.join(INDEX);
        path.set_extension(EXT);

        if let Some(parent) = path.parent() {
            if !parent.is_dir() {
                fs::create_dir_all(parent)?;
            }
        }

        debug!("+index: {}", path.display());

        let mut f = fs::File::create(path)?;
        f.write_all(&out.into_bytes())?;

        Ok(())
    }

    fn section_title(&self, out: &mut FmtWrite, ty: &str, name: &str) -> Result<()> {
        write!(out, "<h1>")?;
        write!(out, "{name}", name = name)?;
        write!(out, "<span class=\"type\">{}</span>", ty)?;
        write!(out, "</h1>")?;

        Ok(())
    }
}

pub struct Collector {
    buffer: String,
}

impl Collecting for Collector {
    type Processor = Processor;

    fn new() -> Self {
        Collector { buffer: String::new() }
    }

    fn into_bytes(self, processor: &Self::Processor) -> Result<Vec<u8>> {
        let mut out = String::new();

        processor.write_doc(&mut out, move |out| {
                out.write_str(&self.buffer)?;
                Ok(())
            })?;

        Ok(out.into_bytes())
    }
}

impl FmtWrite for Collector {
    fn write_str(&mut self, other: &str) -> ::std::result::Result<(), ::std::fmt::Error> {
        self.buffer.write_str(other)
    }
}

impl PackageProcessor for Processor {
    type Out = Collector;

    fn ext(&self) -> &str {
        EXT
    }

    fn env(&self) -> &Environment {
        &self.env
    }

    fn package_prefix(&self) -> &Option<RpPackage> {
        &self.package_prefix
    }

    fn out_path(&self) -> &Path {
        &self.out_path
    }

    fn default_process(&self, out: &mut Self::Out, type_id: &RpTypeId, _: &RpPos) -> Result<()> {
        let type_id = type_id.clone();

        write!(out, "<h1>unknown `{:?}`</h1>\n", &type_id)?;

        Ok(())
    }

    fn resolve_full_path(&self, root_dir: &Path, package: RpPackage) -> PathBuf {
        let mut full_path = root_dir.to_owned();
        full_path = full_path.join(self.package_file(package));
        full_path.set_extension(self.ext());
        full_path
    }

    fn process_service(&self,
                       out: &mut Self::Out,
                       _: &RpTypeId,
                       _: &RpPos,
                       body: Rc<RpServiceBody>)
                       -> Result<()> {
        write!(out,
               "<section id=\"{}\" class=\"section-service\">",
               body.name)?;

        self.section_title(out, "service", &body.name)?;
        self.write_description(out, &body.comment)?;

        write!(out, "</section>")?;
        Ok(())
    }

    fn process_enum(&self,
                    out: &mut Self::Out,
                    _: &RpTypeId,
                    _: &RpPos,
                    body: Rc<RpEnumBody>)
                    -> Result<()> {
        write!(out, "<section id=\"{}\" class=\"section-enum\">", body.name)?;

        self.section_title(out, "enum", &body.name)?;
        self.write_description(out, &body.comment)?;
        self.write_variants(out, body.variants.iter())?;

        write!(out, "</section>")?;
        Ok(())
    }

    fn process_interface(&self,
                         out: &mut Self::Out,
                         type_id: &RpTypeId,
                         _: &RpPos,
                         body: Rc<RpInterfaceBody>)
                         -> Result<()> {
        write!(out,
               "<section id=\"{}\" class=\"section-interface\">",
               body.name)?;

        self.section_title(out, "interface", &body.name)?;
        self.write_description(out, &body.comment)?;

        for (name, sub_type) in &body.sub_types {
            let id = format!("{}_{}", body.name, sub_type.name);
            write!(out, "<h2 id=\"{id}\">{name}</h2>", id = id, name = name)?;

            let fields = body.fields.iter().chain(sub_type.fields.iter());

            self.write_description(out, &sub_type.comment)?;
            self.write_fields(out, type_id, fields)?;
        }

        write!(out, "</section>")?;
        Ok(())
    }

    fn process_type(&self,
                    out: &mut Self::Out,
                    type_id: &RpTypeId,
                    _: &RpPos,
                    body: Rc<RpTypeBody>)
                    -> Result<()> {
        write!(out, "<section id=\"{}\" class=\"section-type\">", body.name)?;

        self.section_title(out, "type", &body.name)?;
        self.write_description(out, &body.comment)?;
        self.write_fields(out, type_id, body.fields.iter())?;

        write!(out, "</section>")?;
        Ok(())
    }

    fn process_tuple(&self,
                     out: &mut Self::Out,
                     type_id: &RpTypeId,
                     _: &RpPos,
                     body: Rc<RpTupleBody>)
                     -> Result<()> {
        write!(out,
               "<section id=\"{}\" class=\"section-tuple\">",
               body.name)?;

        self.section_title(out, "tuple", &body.name)?;
        self.write_description(out, &body.comment)?;
        self.write_fields(out, type_id, body.fields.iter())?;

        write!(out, "</section>")?;
        Ok(())
    }
}

impl Backend for Processor {
    fn process(&self) -> Result<()> {
        let files = self.populate_files()?;
        self.write_stylesheets()?;
        self.write_index(files.keys().map(|p| *p))?;
        self.write_files(files)?;
        Ok(())
    }

    fn verify(&self) -> Result<Vec<Error>> {
        Ok(vec![])
    }
}
