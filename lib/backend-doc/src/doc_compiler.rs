//! Compiler for generating documentation.

use super::{DOC_CSS_NAME, NORMALIZE_CSS_NAME};
use core::errors::*;
use core::flavored::{RpDecl, RpFile, RpVersionedPackage};
use core::{AsPackage, CoreFlavor};
use doc_builder::DocBuilder;
use enum_processor::EnumProcessor;
use genco::IoFmt;
use index_processor::{Data as IndexData, IndexProcessor};
use interface_processor::InterfaceProcessor;
use package_processor::{Data as PackageData, PackageProcessor};
use processor::Processor;
use service_processor::ServiceProcessor;
use std::cell::RefCell;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use syntect::highlighting::Theme;
use syntect::parsing::SyntaxSet;
use trans::Translated;
use tuple_processor::TupleProcessor;
use type_processor::TypeProcessor;

const NORMALIZE_CSS: &[u8] = include_bytes!("static/normalize.css");

pub struct DocCompiler<'a> {
    pub env: Translated<CoreFlavor>,
    pub out_path: PathBuf,
    pub skip_static: bool,
    pub theme_css: &'a [u8],
    pub syntax_theme: &'a Theme,
    pub syntax_set: &'a SyntaxSet,
}

impl<'a> DocCompiler<'a> {
    /// Do the compilation.
    pub fn compile(&self) -> Result<()> {
        for (_, file) in self.env.for_each_file() {
            for decl in file.for_each_decl() {
                self.process_decl(decl)?;
            }
        }

        self.write_index(self.env.for_each_file())?;

        for (package, file) in self.env.for_each_file() {
            self.write_package(package, file)?;
        }

        if !self.skip_static {
            self.write_stylesheets()?;
        }

        Ok(())
    }

    /// Process a single declaration.
    fn process_decl(&self, decl: &RpDecl) -> Result<()> {
        use core::RpDecl::*;

        let package = decl.name().package.try_as_package()?;

        // maintain to know where to import static resources from.
        let mut root = Vec::new();
        let mut path = self.out_path.to_owned();

        for part in package.parts() {
            root.push("..");
            path = path.join(part.as_str());

            if !path.is_dir() {
                debug!("+dir: {}", path.display());
                fs::create_dir_all(&path)?;
            }
        }

        let name = decl.name().parts.join(".");

        // complete path to root and static resources
        let root = root.join("/");

        let out = path.join(format!("{}.{}.html", decl.kind(), name));
        debug!("+file: {}", out.display());
        let mut f = File::create(&out)?;
        let mut fmt = IoFmt(&mut f);
        let out = RefCell::new(DocBuilder::new(&mut fmt));

        match *decl {
            Interface(ref body) => InterfaceProcessor {
                out: out,
                env: &self.env,
                syntax: (self.syntax_theme, self.syntax_set),
                root: &root,
                body: body,
            }.process(),
            Type(ref body) => TypeProcessor {
                out: out,
                env: &self.env,
                syntax: (self.syntax_theme, self.syntax_set),
                root: &root,
                body: body,
            }.process(),
            Tuple(ref body) => TupleProcessor {
                out: out,
                env: &self.env,
                syntax: (self.syntax_theme, self.syntax_set),
                root: &root,
                body: body,
            }.process(),
            Enum(ref body) => EnumProcessor {
                out: out,
                env: &self.env,
                syntax: (self.syntax_theme, self.syntax_set),
                root: &root,
                body: body,
            }.process(),
            Service(ref body) => ServiceProcessor {
                out: out,
                env: &self.env,
                syntax: (self.syntax_theme, self.syntax_set),
                root: &root,
                body: body,
            }.process(),
        }
    }

    /// Write stylesheets.
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

        debug!("+css: {}", doc_css.display());
        let mut f = fs::File::create(doc_css)?;
        f.write_all(self.theme_css)?;

        Ok(())
    }

    /// Write the package index file index file.
    fn write_package(&self, package: &RpVersionedPackage, file: &RpFile) -> Result<()> {
        let mut path = self.out_path.to_owned();

        let mut root = Vec::new();

        for part in package.to_package(|v| v.to_string()).parts() {
            root.push("..");
            path = path.join(part);
        }

        let index_html = path.join("index.html");
        let mut f = File::create(&index_html)?;

        PackageProcessor {
            out: RefCell::new(DocBuilder::new(&mut IoFmt(&mut f))),
            env: &self.env,
            syntax: (self.syntax_theme, self.syntax_set),
            root: &root.join("/"),
            body: &PackageData {
                package: package,
                file: file,
            },
        }.process()?;

        debug!("+file: {}", index_html.display());
        Ok(())
    }

    /// Write the root index file.
    fn write_index<'it, I>(&self, entries: I) -> Result<()>
    where
        I: IntoIterator<Item = (&'it RpVersionedPackage, &'it RpFile)>,
    {
        let index_html = self.out_path.join("index.html");
        let mut f = File::create(&index_html)?;

        let entries = entries.into_iter().collect();

        IndexProcessor {
            out: RefCell::new(DocBuilder::new(&mut IoFmt(&mut f))),
            env: &self.env,
            syntax: (self.syntax_theme, self.syntax_set),
            root: &".",
            body: &IndexData { entries: entries },
        }.process()?;

        debug!("+file: {}", index_html.display());
        Ok(())
    }
}
