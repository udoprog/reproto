use backend::*;
use backend::collecting::Collecting;
use backend::errors::*;
use backend::package_processor::PackageProcessor;
use core::*;
use serde_json;
use std::fmt::Write as FmtWrite;
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

const EXT: &str = "json";

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

    fn package_file(&self, package: RpPackage) -> String {
        package.parts.join("_")
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

    fn into_bytes(self, _: &Self::Processor) -> Result<Vec<u8>> {
        Ok(self.buffer.into_bytes())
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

    fn default_process(&self, _: &mut Self::Out, _: &RpTypeId, _: &RpPos) -> Result<()> {
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
        writeln!(out, "{}", serde_json::to_string(&body)?)?;
        Ok(())
    }

    fn process_enum(&self,
                    out: &mut Self::Out,
                    _: &RpTypeId,
                    _: &RpPos,
                    body: Rc<RpEnumBody>)
                    -> Result<()> {
        writeln!(out, "{}", serde_json::to_string(&body)?)?;
        Ok(())
    }

    fn process_interface(&self,
                         out: &mut Self::Out,
                         _: &RpTypeId,
                         _: &RpPos,
                         body: Rc<RpInterfaceBody>)
                         -> Result<()> {
        writeln!(out, "{}", serde_json::to_string(&body)?)?;
        Ok(())
    }

    fn process_type(&self,
                    out: &mut Self::Out,
                    _: &RpTypeId,
                    _: &RpPos,
                    body: Rc<RpTypeBody>)
                    -> Result<()> {
        writeln!(out, "{}", serde_json::to_string(&body)?)?;
        Ok(())
    }

    fn process_tuple(&self,
                     out: &mut Self::Out,
                     _: &RpTypeId,
                     _: &RpPos,
                     body: Rc<RpTupleBody>)
                     -> Result<()> {
        writeln!(out, "{}", serde_json::to_string(&body)?)?;
        Ok(())
    }
}

impl Backend for Processor {
    fn process(&self) -> Result<()> {
        let files = self.populate_files()?;
        self.write_files(files)?;
        Ok(())
    }

    fn verify(&self) -> Result<Vec<Error>> {
        Ok(vec![])
    }
}
