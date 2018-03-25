use IntoBytes;
use core::errors::*;
use core::{Flavor, Handle, RelativePath, RelativePathBuf, RpDecl, RpEnumBody, RpInterfaceBody,
           RpName, RpPackage, RpServiceBody, RpTupleBody, RpType, RpTypeBody, RpVersionedPackage,
           WithPos};
use std::collections::BTreeMap;
use std::io::Write;

pub trait PackageProcessor<'el, F: 'static>
where
    Self: 'el + Sized,
    F: Flavor<Type = RpType>,
{
    type Out: Default + IntoBytes<Self>;
    type DeclIter: Iterator<Item = &'el RpDecl<F>>;

    fn ext(&self) -> &str;

    /// Iterate over all existing declarations.
    fn decl_iter(&self) -> Self::DeclIter;

    fn handle(&self) -> &'el Handle;

    fn default_process(&self, _: &mut Self::Out, name: &'el RpName) -> Result<()> {
        warn!("not supported: {}", name);
        Ok(())
    }

    fn processed_package(&self, package: &RpVersionedPackage) -> RpPackage;

    fn process_interface(&self, out: &mut Self::Out, body: &'el RpInterfaceBody<F>) -> Result<()> {
        self.default_process(out, &body.name)
    }

    fn process_type(&self, out: &mut Self::Out, body: &'el RpTypeBody<F>) -> Result<()> {
        self.default_process(out, &body.name)
    }

    fn process_tuple(&self, out: &mut Self::Out, body: &'el RpTupleBody<F>) -> Result<()> {
        self.default_process(out, &body.name)
    }

    fn process_enum(&self, out: &mut Self::Out, body: &'el RpEnumBody<F>) -> Result<()> {
        self.default_process(out, &body.name)
    }

    fn process_service(&self, out: &mut Self::Out, body: &'el RpServiceBody<F>) -> Result<()> {
        self.default_process(out, &body.name)
    }

    fn populate_files(&self) -> Result<BTreeMap<RpVersionedPackage, Self::Out>> {
        self.do_populate_files(|_| Ok(()))
    }

    fn do_populate_files<C>(
        &self,
        mut callback: C,
    ) -> Result<BTreeMap<RpVersionedPackage, Self::Out>>
    where
        C: FnMut(&'el RpDecl<F>) -> Result<()>,
    {
        use self::RpDecl::*;

        let mut files = BTreeMap::new();

        // Process all types discovered so far.
        for decl in self.decl_iter() {
            callback(decl)
                .and_then(|_| {
                    let mut out = files
                        .entry(decl.name().package.clone())
                        .or_insert_with(Self::Out::default);

                    match *decl {
                        Interface(ref b) => self.process_interface(&mut out, b),
                        Type(ref b) => self.process_type(&mut out, b),
                        Tuple(ref b) => self.process_tuple(&mut out, b),
                        Enum(ref b) => self.process_enum(&mut out, b),
                        Service(ref b) => self.process_service(&mut out, b),
                    }
                })
                .with_pos(decl.pos())?;
        }

        Ok(files)
    }

    fn resolve_full_path(&self, package: &RpPackage) -> Result<RelativePathBuf> {
        let mut full_path = package
            .parts
            .iter()
            .fold(RelativePathBuf::new(), |a, b| a.join(b));
        full_path.set_extension(self.ext());
        Ok(full_path)
    }

    fn setup_module_path(&self, package: &RpPackage) -> Result<RelativePathBuf> {
        let handle = self.handle();
        let full_path = self.resolve_full_path(package)?;

        {
            let parent = full_path.parent().unwrap_or(RelativePath::new("."));

            if !handle.is_dir(&parent) {
                debug!("+dir: {}", parent.display());
                handle.create_dir_all(&parent)?;
            }
        }

        Ok(full_path)
    }

    fn write_files(&'el self, files: BTreeMap<RpVersionedPackage, Self::Out>) -> Result<()> {
        let handle = self.handle();

        for (package, out) in files {
            let package = self.processed_package(&package);
            let full_path = self.setup_module_path(&package)?;

            debug!("+module: {}", full_path.display());

            let mut f = handle.create(&full_path)?;
            let bytes = out.into_bytes(self, &package)?;
            f.write_all(&bytes)?;
            f.flush()?;
        }

        Ok(())
    }
}
