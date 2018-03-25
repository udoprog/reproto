//! Helper component to build C# files.

use core::errors::*;
use core::{Handle, RelativePathBuf};
use genco::csharp::Extra;
use genco::{Csharp, IoFmt, Tokens, WriteTokens};

pub struct CsharpFile<'el, F> {
    namespace: &'el str,
    class_name: &'el str,
    builder: F,
}

impl<'el, F> CsharpFile<'el, F>
where
    F: FnOnce(&mut Tokens<'el, Csharp<'el>>) -> Result<()>,
{
    pub fn new(namespace: &'el str, class_name: &'el str, builder: F) -> CsharpFile<'el, F> {
        CsharpFile {
            namespace: namespace,
            class_name: class_name,
            builder: builder,
        }
    }

    pub fn process(self, handle: &Handle) -> Result<()> {
        let parts = self.namespace.split('.').collect::<Vec<_>>();

        let path = parts
            .iter()
            .cloned()
            .fold(RelativePathBuf::new(), |p, part| p.join(part));

        if !handle.is_dir(&path) {
            debug!("+dir: {}", path.display());
            handle.create_dir_all(&path)?;
        }

        let path = path.join(format!("{}.cs", self.class_name));

        let mut file: Tokens<Csharp> = Tokens::new();
        (self.builder)(&mut file)?;

        let mut extra = Extra::default();
        extra.namespace(self.namespace);

        debug!("+class: {}", path.display());
        IoFmt(&mut handle.create(&path)?.as_mut()).write_file(file, &mut extra)?;

        Ok(())
    }
}
