use super::RpVersionedPackage;
use serde;
use std::collections::HashMap;
use std::fmt;
use std::mem;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RpPackage {
    pub parts: Vec<String>,
}

impl RpPackage {
    pub fn new(parts: Vec<String>) -> RpPackage {
        RpPackage { parts: parts }
    }

    /// Parse a package from a string.
    ///
    /// Warning: This does not perform any validation that the given package only contains
    /// identifier components!
    ///
    /// * An empty string results in a package _without_ any parts.
    /// * All other strings are split on dots.
    pub fn parse(input: &str) -> RpPackage {
        if input.is_empty() {
            return Self::empty();
        }

        RpPackage::new(input.split('.').map(ToOwned::to_owned).collect())
    }

    /// Build an empty package.
    pub fn empty() -> RpPackage {
        RpPackage { parts: vec![] }
    }

    /// Join this package with another, versioned, package.
    pub fn join_versioned(&self, other: &RpVersionedPackage) -> RpVersionedPackage {
        let mut parts = self.parts.clone();
        parts.extend(other.package.parts.clone());
        RpVersionedPackage::new(RpPackage::new(parts), other.version.clone())
    }

    /// Join this package with another.
    pub fn join(&self, other: &RpPackage) -> RpPackage {
        let mut parts = self.parts.clone();
        parts.extend(other.parts.clone());
        RpPackage::new(parts)
    }

    /// Join with the given part.
    pub fn join_part<S: AsRef<str>>(mut self, other: S) -> RpPackage {
        self.parts.push(other.as_ref().to_string());
        self
    }

    /// Check if this package starts with another package.
    pub fn starts_with(&self, other: &RpPackage) -> bool {
        if self.parts.len() < other.parts.len() {
            return false;
        }

        self.parts.iter().zip(other.parts.iter()).all(
            |(a, b)| a == b,
        )
    }

    /// Replace all keyword components in this package.
    pub fn with_replacements(mut self, keywords: &HashMap<String, String>) -> Self {
        for p in self.parts.iter_mut() {
            if let Some(keyword) = keywords.get(p.as_str()) {
                mem::replace(p, keyword.to_string());
            }
        }

        self
    }

    /// Apply the given naming policy to each part.
    pub fn with_naming<N>(mut self, naming: N) -> Self
    where
        N: Fn(&str) -> String,
    {
        for p in self.parts.iter_mut() {
            let new_name = naming(p);
            mem::replace(p, new_name);
        }

        self
    }
}

impl fmt::Display for RpPackage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.parts.join("."))
    }
}

impl serde::Serialize for RpPackage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> serde::Deserialize<'de> for RpPackage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct RpPackageVisitor;

        impl<'de> serde::de::Visitor<'de> for RpPackageVisitor {
            type Value = RpPackage;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a SemVer version as a string")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(RpPackage::parse(v))
            }
        }

        deserializer.deserialize_str(RpPackageVisitor)
    }
}
