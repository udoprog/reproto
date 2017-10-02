use super::*;
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub struct PythonCompiler<'a> {
    pub out_path: PathBuf,
    pub backend: &'a PythonBackend,
}

impl<'a> PythonCompiler<'a> {
    pub fn compile(&self) -> Result<()> {
        let files = self.populate_files()?;
        self.write_files(files)
    }
}

impl<'a> PackageProcessor<'a> for PythonCompiler<'a> {
    type Out = PythonFileSpec;

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

    fn process_tuple(
        &self,
        out: &mut Self::Out,
        name: &RpName,
        body: Rc<Loc<RpTupleBody>>,
    ) -> Result<()> {
        self.backend.process_tuple(out, name, body)
    }

    fn process_enum(
        &self,
        out: &mut Self::Out,
        name: &RpName,
        body: Rc<Loc<RpEnumBody>>,
    ) -> Result<()> {
        self.backend.process_enum(out, name, body)
    }

    fn process_type(
        &self,
        out: &mut Self::Out,
        name: &RpName,
        body: Rc<Loc<RpTypeBody>>,
    ) -> Result<()> {
        self.backend.process_type(out, name, body)
    }

    fn process_interface(
        &self,
        out: &mut Self::Out,
        name: &RpName,
        body: Rc<Loc<RpInterfaceBody>>,
    ) -> Result<()> {
        self.backend.process_interface(out, name, body)
    }

    fn populate_files(&self) -> Result<BTreeMap<RpVersionedPackage, PythonFileSpec>> {
        let mut enums = Vec::new();

        let mut files = self.do_populate_files(|name, decl| {
            if let RpDecl::Enum(ref body) = **decl {
                enums.push((name.clone(), body.clone()));
            }

            Ok(())
        })?;

        for (name, body) in enums {
            if let Some(ref mut file_spec) = files.get_mut(&name.package) {
                file_spec.0.push(self.backend.enum_variants(&body)?);
            } else {
                return Err(format!("no such package: {}", &name.package).into());
            }
        }

        Ok(files)
    }

    fn resolve_full_path(&self, package: &RpPackage) -> Result<PathBuf> {
        let mut full_path = self.out_path().to_owned();
        let mut iter = package.parts.iter().peekable();

        while let Some(part) = iter.next() {
            full_path = full_path.join(part);

            if iter.peek().is_none() {
                continue;
            }

            if !full_path.is_dir() {
                debug!("+dir: {}", full_path.display());
                fs::create_dir_all(&full_path)?;
            }

            let init_path = full_path.join(INIT_PY);

            if !init_path.is_file() {
                debug!("+init: {}", init_path.display());
                File::create(init_path)?;
            }
        }

        // path to final file
        full_path.set_extension(self.ext());
        Ok(full_path)
    }
}
