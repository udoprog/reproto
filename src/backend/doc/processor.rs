use backend::*;
use backend::collecting::Collecting;
use backend::errors::*;
use backend::package_processor::PackageProcessor;
use core::*;
use pulldown_cmark as markdown;
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
    listeners: Box<Listeners>,
}

const EXT: &str = "html";
const INDEX: &str = "index";

const NORMALIZE_CSS_NAME: &str = "normalize.css";
const NORMALIZE_CSS: &[u8] = include_bytes!("static/normalize.css");

const DOC_CSS_NAME: &str = "doc.css";
const DOC_CSS: &[u8] = include_bytes!("static/doc.css");

impl Processor {
    pub fn new(_options: ProcessorOptions,
               env: Environment,
               out_path: PathBuf,
               package_prefix: Option<RpPackage>,
               listeners: Box<Listeners>)
               -> Processor {
        Processor {
            env: env,
            out_path: out_path,
            package_prefix: package_prefix,
            listeners: listeners,
        }
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

    fn write_variants<'a, I>(out: &mut FmtWrite, variants: I) -> Result<()>
        where I: Iterator<Item = &'a RpLoc<Rc<RpEnumVariant>>>
    {
        write!(out, "<div class=\"variants\">")?;

        for variant in variants {
            write!(out, "<h4 class=\"name\">{}</h4>", variant.name)?;
            // write!(out, "<div><pre>{:?}</pre></div>", variant.arguments)?;
        }

        write!(out, "</div>")?;

        Ok(())
    }

    fn write_fields<'a, I>(out: &mut FmtWrite, fields: I) -> Result<()>
        where I: Iterator<Item = &'a RpLoc<RpField>>
    {
        write!(out, "<div class=\"fields\">")?;

        for field in fields {
            let comment = field.comment.join("\n");

            write!(out, "<div class=\"name\"><b>{}</b></div>", field.name)?;
            write!(out,
                   "<div class=\"description\">{}</div>",
                   Self::markdown(&comment))?;
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

        if !normalize_css.is_file() {
            debug!("+css: {}", normalize_css.display());
            let mut f = fs::File::create(normalize_css)?;
            f.write_all(NORMALIZE_CSS)?;
        }

        let doc_css = self.out_path.join(DOC_CSS_NAME);

        if !doc_css.is_file() {
            debug!("+css: {}", doc_css.display());
            let mut f = fs::File::create(doc_css)?;
            f.write_all(DOC_CSS)?;
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
}

impl Collecting for String {
    type Processor = Processor;

    fn new() -> Self {
        String::new()
    }

    fn into_bytes(self, processor: &Self::Processor) -> Result<Vec<u8>> {
        let mut out = String::new();

        processor.write_doc(&mut out, move |out| {
                out.write_str(&self)?;
                Ok(())
            })?;

        Ok(out.into_bytes())
    }
}

impl PackageProcessor for Processor {
    type Out = String;

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

    fn process_enum(&self,
                    out: &mut Self::Out,
                    _: &RpTypeId,
                    _: &RpPos,
                    body: Rc<RpEnumBody>)
                    -> Result<()> {
        write!(out, "<h1>enum {}</h1>\n", body.name)?;

        if !body.variants.is_empty() {
            write!(out, "<h2>variants</h2>")?;
            Self::write_variants(out, body.variants.iter())?;
        }

        Ok(())
    }

    fn process_interface(&self,
                         out: &mut Self::Out,
                         _: &RpTypeId,
                         _: &RpPos,
                         body: Rc<RpInterfaceBody>)
                         -> Result<()> {
        write!(out, "<h1>interface {}</h1>\n", body.name)?;

        for (name, sub_type) in &body.sub_types {
            write!(out, "<h2>sub type: {}</h2>", name)?;
            let fields = body.fields.iter().chain(sub_type.fields.iter());
            Self::write_fields(out, fields)?;
        }

        Ok(())
    }

    fn process_type(&self,
                    out: &mut Self::Out,
                    _: &RpTypeId,
                    _: &RpPos,
                    body: Rc<RpTypeBody>)
                    -> Result<()> {
        write!(out, "<h1>type {}</h1>\n", body.name)?;

        for field in &body.fields {
            write!(out, "<h3>{}</h3>\n", field.name)?;

            let comment = field.comment.join("\n");
            out.write_str(&Self::markdown(&comment))?;
        }

        Ok(())
    }

    fn process_tuple(&self,
                     out: &mut Self::Out,
                     _: &RpTypeId,
                     _: &RpPos,
                     body: Rc<RpTupleBody>)
                     -> Result<()> {
        write!(out, "<h1>tuple {}</h1>\n", body.name)?;

        for field in &body.fields {
            write!(out, "<h3>{}</h3>\n", field.name)?;

            let comment = field.comment.join("\n");
            out.write_str(&Self::markdown(&comment))?;
        }

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
