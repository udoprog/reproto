//! Helper component to build Java files.

use core::errors::*;
use core::{Handle, RelativePathBuf};
use genco::java::Extra;
use genco::{IoFmt, Java, Tokens, WriteTokens};

pub struct JavaFile<'el, F> {
    package: &'el str,
    class_name: &'el str,
    builder: F,
}

impl<'el, F> JavaFile<'el, F>
where
    F: FnOnce(&mut Tokens<'el, Java<'el>>) -> Result<()>,
{
    pub fn new(package: &'el str, class_name: &'el str, builder: F) -> JavaFile<'el, F> {
        JavaFile {
            package: package,
            class_name: class_name,
            builder: builder,
        }
    }

    pub fn process(self, handle: &Handle) -> Result<()> {
        let parts = self.package.split('.').collect::<Vec<_>>();

        let path = parts
            .iter()
            .cloned()
            .fold(RelativePathBuf::new(), |p, part| p.join(part));

        if !handle.is_dir(&path) {
            debug!("+dir: {}", path.display());
            handle.create_dir_all(&path)?;
        }

        let path = path.join(format!("{}.java", self.class_name));

        let mut file: Tokens<Java> = Tokens::new();
        (self.builder)(&mut file)?;

        let mut extra = Extra::default();
        extra.package(self.package);

        debug!("+class: {}", path.display());
        IoFmt(&mut handle.create(&path)?.as_mut()).write_file(file, &mut extra)?;

        Ok(())
    }
}
