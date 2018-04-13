//! Describes a fully qualified name as a model

use errors::Result;
use serde::Serialize;
use std::fmt;
use {CoreFlavor, Flavor, Loc, Translate, Translator};

#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(bound = "F::Package: Serialize")]
pub struct RpName<F: 'static>
where
    F: Flavor,
{
    /// Alias used if the name was imported from another package.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<Loc<String>>,
    /// Package that name belongs to.
    pub package: F::Package,
    /// Absolute parts of the name, from the root of the package.
    pub parts: Vec<String>,
}

impl<F: 'static> RpName<F>
where
    F: Flavor,
{
    pub fn new(prefix: Option<Loc<String>>, package: F::Package, parts: Vec<String>) -> Self {
        Self {
            prefix: prefix,
            package: package,
            parts: parts,
        }
    }

    pub fn extend<I>(&self, it: I) -> Self
    where
        I: IntoIterator<Item = String>,
    {
        let mut parts = self.parts.clone();
        parts.extend(it);

        Self {
            prefix: self.prefix.clone(),
            package: self.package.clone(),
            parts: parts,
        }
    }

    pub fn push(&self, part: String) -> Self {
        let mut parts = self.parts.clone();
        parts.push(part);

        Self {
            prefix: self.prefix.clone(),
            package: self.package.clone(),
            parts: parts,
        }
    }

    pub fn join<S: AsRef<str>>(&self, joiner: S) -> String {
        self.parts.join(joiner.as_ref())
    }

    /// Convert to a name without a prefix component.
    pub fn without_prefix(self) -> Self {
        Self {
            prefix: None,
            package: self.package,
            parts: self.parts,
        }
    }

    pub fn with_package(self, package: F::Package) -> Self {
        Self {
            prefix: self.prefix,
            package: package,
            parts: self.parts,
        }
    }

    /// Build a new name out if the given paths.
    pub fn with_parts(self, parts: Vec<String>) -> Self {
        Self {
            prefix: self.prefix,
            package: self.package,
            parts: parts,
        }
    }

    pub fn is_same(&self, other: &Self) -> bool {
        self.package == other.package && self.parts == other.parts
    }
}

impl RpName<CoreFlavor> {
    /// Convert to a name without a version component.
    pub fn without_version(self) -> Self {
        Self {
            prefix: self.prefix,
            package: self.package.without_version(),
            parts: self.parts,
        }
    }

    /// Localize name.
    ///
    /// Strips version of any type which is _not_ imported.
    pub fn localize(self) -> Self {
        if self.prefix.is_some() {
            return self;
        }

        self.without_version()
    }
}

impl<F: 'static> fmt::Display for RpName<F>
where
    F: Flavor,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(ref prefix) = self.prefix {
            write!(f, "{}::{}", prefix, self.parts.join("::"))
        } else {
            write!(f, "{}", self.parts.join("::"))
        }
    }
}

impl<F: 'static, T> Translate<T> for RpName<F>
where
    F: Flavor,
    T: Translator<Source = F>,
{
    type Source = F;
    type Out = RpName<T::Target>;

    /// Translate into different flavor.
    fn translate(self, translator: &T) -> Result<RpName<T::Target>> {
        Ok(RpName {
            prefix: self.prefix,
            package: translator.translate_package(self.package)?,
            parts: self.parts,
        })
    }
}
