//! Helper utilities for processors.

use core::{RpChannel, RpEndpoint, RpPackage, RpVersionedPackage};
use core::errors::*;

pub trait Processor {
    /// Build the java package of a given package.
    ///
    /// This includes the prefixed configured in `self.options`, if specified.
    fn java_package(&self, pkg: &RpVersionedPackage) -> RpPackage {
        pkg.as_package(|version| {
            format!("_{}", version).replace(".", "_").replace("-", "_")
        })
    }

    /// Extract endpoint request.
    ///
    /// Errors if more than one argument is present.
    fn endpoint_request<'a>(
        &self,
        endpoint: &'a RpEndpoint,
    ) -> Result<Option<(&'a str, &'a RpChannel)>> {
        let mut it = endpoint.arguments.iter();

        if let Some(arg) = it.next() {
            return Ok(Some((arg.ident.as_str(), &arg.channel)));
        }

        Ok(None)
    }
}
