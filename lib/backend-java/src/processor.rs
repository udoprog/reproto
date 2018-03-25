//! Helper utilities for processors.

use flavored::{RpPackage, RpVersionedPackage};

pub trait Processor {
    /// Build the java package of a given package.
    ///
    /// This includes the prefixed configured in `self.options`, if specified.
    fn java_package(&self, pkg: &RpVersionedPackage) -> RpPackage {
        pkg.as_package(|version| format!("_{}", version).replace(".", "_").replace("-", "_"))
    }
}
