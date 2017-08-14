use collecting::Collecting;
use core::{Loc, Pos, RpDecl, RpEnumBody, RpInterfaceBody, RpPackage, RpServiceBody, RpTupleBody,
           RpTypeBody, RpTypeId, RpVersionedPackage};
use environment::Environment;
use errors::*;
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

pub trait PackageProcessor<'a>
where
    Self: 'a + Sized,
{
    type Out: Collecting<'a, Processor = Self>;

    fn ext(&self) -> &str;

    fn env(&self) -> &Environment;

    fn out_path(&self) -> &Path;

    fn default_process(&self, _: &mut Self::Out, type_id: &RpTypeId, _: &Pos) -> Result<()> {
        warn!("not supported: {}", type_id);
        Ok(())
    }

    fn processed_package(&self, package: &RpVersionedPackage) -> RpPackage;

    fn process_interface(
        &self,
        out: &mut Self::Out,
        type_id: &RpTypeId,
        pos: &Pos,
        _: Rc<RpInterfaceBody>,
    ) -> Result<()> {
        self.default_process(out, type_id, pos)
    }

    fn process_type(
        &self,
        out: &mut Self::Out,
        type_id: &RpTypeId,
        pos: &Pos,
        _: Rc<RpTypeBody>,
    ) -> Result<()> {
        self.default_process(out, type_id, pos)
    }

    fn process_tuple(
        &self,
        out: &mut Self::Out,
        type_id: &RpTypeId,
        pos: &Pos,
        _: Rc<RpTupleBody>,
    ) -> Result<()> {
        self.default_process(out, type_id, pos)
    }

    fn process_enum(
        &self,
        out: &mut Self::Out,
        type_id: &RpTypeId,
        pos: &Pos,
        _: Rc<RpEnumBody>,
    ) -> Result<()> {
        self.default_process(out, type_id, pos)
    }

    fn process_service(
        &self,
        out: &mut Self::Out,
        type_id: &RpTypeId,
        pos: &Pos,
        _: Rc<RpServiceBody>,
    ) -> Result<()> {
        self.default_process(out, type_id, pos)
    }

    fn populate_files(&self) -> Result<BTreeMap<RpVersionedPackage, Self::Out>> {
        self.do_populate_files(|_, _| Ok(()))
    }

    fn do_populate_files<F>(
        &self,
        mut callback: F,
    ) -> Result<BTreeMap<RpVersionedPackage, Self::Out>>
    where
        F: FnMut(Rc<RpTypeId>, Rc<Loc<RpDecl>>) -> Result<()>,
    {
        use self::RpDecl::*;

        let mut files = BTreeMap::new();

        // Process all types discovered so far.
        self.env().for_each_decl(|type_id, decl| {
            callback(type_id.clone(), decl.clone())?;

            let mut out = files.entry(type_id.package.clone()).or_insert_with(
                Self::Out::new,
            );

            match **decl {
                Interface(ref body) => {
                    self.process_interface(
                        &mut out,
                        type_id.as_ref(),
                        decl.pos(),
                        body.clone(),
                    )?;
                }
                Type(ref body) => {
                    self.process_type(
                        &mut out,
                        type_id.as_ref(),
                        decl.pos(),
                        body.clone(),
                    )?;
                }
                Tuple(ref body) => {
                    self.process_tuple(
                        &mut out,
                        type_id.as_ref(),
                        decl.pos(),
                        body.clone(),
                    )?;
                }
                Enum(ref body) => {
                    self.process_enum(
                        &mut out,
                        type_id.as_ref(),
                        decl.pos(),
                        body.clone(),
                    )?;
                }
                Service(ref body) => {
                    self.process_service(
                        &mut out,
                        type_id.as_ref(),
                        decl.pos(),
                        body.clone(),
                    )?;
                }
            };

            Ok(())
        })?;

        Ok(files)
    }

    fn resolve_full_path(&self, package: &RpPackage) -> Result<PathBuf> {
        let full_path = self.out_path().to_owned();
        let mut full_path = package.parts.iter().fold(full_path, |a, b| a.join(b));
        full_path.set_extension(self.ext());
        Ok(full_path)
    }

    fn setup_module_path(&self, package: &RpPackage) -> Result<PathBuf> {
        let full_path = self.resolve_full_path(package)?;

        if let Some(parent) = full_path.parent() {
            if !parent.is_dir() {
                debug!("+dir: {}", parent.display());
                fs::create_dir_all(parent)?;
            }
        }

        Ok(full_path)
    }

    fn write_files(&self, files: BTreeMap<RpVersionedPackage, Self::Out>) -> Result<()> {
        for (package, out) in files {
            let package = self.processed_package(&package);
            let full_path = self.setup_module_path(&package)?;

            debug!("+module: {}", full_path.display());

            let mut f = File::create(full_path)?;
            let bytes = out.into_bytes(self)?;
            f.write_all(&bytes)?;
            f.flush()?;
        }

        Ok(())
    }
}
