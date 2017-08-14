use super::*;
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;

const NORMALIZE_CSS: &[u8] = include_bytes!("static/normalize.css");

pub struct DocCompiler<'a> {
    pub backend: &'a DocBackend,
    pub out_path: PathBuf,
    pub skip_static: bool,
}

impl<'a> DocCompiler<'a> {
    pub fn new(backend: &'a DocBackend, out_path: PathBuf, skip_static: bool) -> DocCompiler {
        DocCompiler {
            backend: backend,
            out_path: out_path,
            skip_static: skip_static,
        }
    }

    pub fn compile(&self) -> Result<()> {
        let mut files = self.populate_files()?;

        if !self.skip_static {
            self.write_stylesheets()?;
        }

        let packages: Vec<_> = files.keys().map(|p| (*p).clone()).collect();
        self.write_index(&packages)?;
        self.write_overviews(&packages, &mut files)?;
        self.write_files(files)?;
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

        let content = self.backend.themes.get(self.backend.theme.as_str());

        if let Some(content) = content {
            debug!("+css: {}", doc_css.display());
            let mut f = fs::File::create(doc_css)?;
            f.write_all(content)?;
        } else {
            return Err(format!("no such theme: {}", &self.backend.theme).into());
        }

        Ok(())
    }

    fn write_index(&self, packages: &[RpVersionedPackage]) -> Result<()> {
        let mut buffer = String::new();

        self.backend.write_doc(
            &mut DefaultDocBuilder::new(&mut buffer),
            |out| {
                self.backend.write_packages(out, packages, None)?;
                Ok(())
            },
        )?;

        let mut path = self.out_path.join(INDEX);
        path.set_extension(EXT);

        if let Some(parent) = path.parent() {
            if !parent.is_dir() {
                fs::create_dir_all(parent)?;
            }
        }

        debug!("+index: {}", path.display());

        let mut f = fs::File::create(path)?;
        f.write_all(&buffer.into_bytes())?;

        Ok(())
    }

    fn write_overviews(
        &self,
        packages: &[RpVersionedPackage],
        files: &mut BTreeMap<RpVersionedPackage, DocCollector>,
    ) -> Result<()> {
        for (package, collector) in files.iter_mut() {
            collector.set_package_title(format!("{}", package));

            {
                let mut new_package = collector.new_package();
                let mut out = DefaultDocBuilder::new(&mut new_package);
                self.backend.write_packages(
                    &mut out,
                    packages,
                    Some(package),
                )?;
            }

            {
                let service_bodies = collector.service_bodies.clone();
                let mut new_service_overview = collector.new_service_overview();
                let mut out = DefaultDocBuilder::new(&mut new_service_overview);
                self.backend.write_service_overview(
                    &mut out,
                    service_bodies,
                )?;
            }

            {
                let decl_bodies = collector.decl_bodies.clone();
                let mut new_type_overview = collector.new_types_overview();
                let mut out = DefaultDocBuilder::new(&mut new_type_overview);
                self.backend.write_types_overview(&mut out, decl_bodies)?;
            }
        }

        Ok(())
    }
}

impl<'a> PackageProcessor<'a> for DocCompiler<'a> {
    type Out = DocCollector;

    fn ext(&self) -> &str {
        EXT
    }

    fn env(&self) -> &Environment {
        &self.backend.env
    }

    fn out_path(&self) -> &Path {
        &self.out_path
    }

    fn processed_package(&self, package: &RpVersionedPackage) -> RpPackage {
        self.backend.package(package)
    }

    fn default_process(&self, _: &mut Self::Out, type_id: &RpTypeId, _: &Pos) -> Result<()> {
        let type_id = type_id.clone();
        warn!("Cannot handle: `{:?}", &type_id);
        Ok(())
    }

    fn resolve_full_path(&self, package: &RpPackage) -> Result<PathBuf> {
        let mut full_path = self.out_path().join(self.backend.package_file(package));
        full_path.set_extension(self.ext());
        Ok(full_path)
    }

    fn process_service(
        &self,
        out: &mut Self::Out,
        type_id: &RpTypeId,
        pos: &Pos,
        body: Rc<RpServiceBody>,
    ) -> Result<()> {
        self.backend.process_service(out, type_id, pos, body)
    }

    fn process_enum(
        &self,
        out: &mut Self::Out,
        type_id: &RpTypeId,
        pos: &Pos,
        body: Rc<RpEnumBody>,
    ) -> Result<()> {
        self.backend.process_enum(out, type_id, pos, body)
    }

    fn process_interface(
        &self,
        out: &mut Self::Out,
        type_id: &RpTypeId,
        pos: &Pos,
        body: Rc<RpInterfaceBody>,
    ) -> Result<()> {
        self.backend.process_interface(out, type_id, pos, body)
    }

    fn process_type(
        &self,
        out: &mut Self::Out,
        type_id: &RpTypeId,
        pos: &Pos,
        body: Rc<RpTypeBody>,
    ) -> Result<()> {
        self.backend.process_type(out, type_id, pos, body)
    }

    fn process_tuple(
        &self,
        out: &mut Self::Out,
        type_id: &RpTypeId,
        pos: &Pos,
        body: Rc<RpTupleBody>,
    ) -> Result<()> {
        self.backend.process_tuple(out, type_id, pos, body)
    }
}
