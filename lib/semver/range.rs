// Copyright 2012-2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use self::Op::{Compatible, Ex, Gt, GtEq, Lt, LtEq, Tilde, Wildcard};
use self::WildcardVersion::{Minor, Patch};
use errors::Error;
use parser;
#[cfg(feature = "serde")]
use serde::de::{self, Deserialize, Deserializer, Visitor};
#[cfg(feature = "serde")]
use serde::ser::{Serialize, Serializer};
use std::fmt;
use std::str;
use version::{Identifier, Version};

/// A `Range` is a struct containing a list of predicates that can apply to ranges of version
/// numbers. Matching operations can then be done with the `Range` against a particular
/// version to see if it satisfies some or all of the constraints.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Range {
    pub predicates: Vec<Predicate>,
}

#[cfg(feature = "serde")]
impl Serialize for Range {
    fn serialize<S>(&self, serializer: S) -> ::std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serialize Range as a string.
        serializer.collect_str(self)
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for Range {
    fn deserialize<D>(deserializer: D) -> ::std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct RangeVisitor;

        /// Deserialize `Range` from a string.
        impl<'de> Visitor<'de> for RangeVisitor {
            type Value = Range;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a semver range without a string")
            }

            fn visit_str<E>(self, v: &str) -> ::std::result::Result<Self::Value, E>
            where
                E: de::Error,
            {
                Range::parse(v).map_err(de::Error::custom)
            }
        }

        deserializer.deserialize_str(RangeVisitor)
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum WildcardVersion {
    Minor,
    Patch,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Op {
    Ex,                        // Exact
    Gt,                        // Greater than
    GtEq,                      // Greater than or equal to
    Lt,                        // Less than
    LtEq,                      // Less than or equal to
    Tilde,                     // e.g. ~1.0.0
    Compatible,                // compatible by definition of semver, indicated by ^
    Wildcard(WildcardVersion), // x.y.*, x.*, *
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Predicate {
    pub op: Op,
    pub major: u64,
    pub minor: Option<u64>,
    pub patch: Option<u64>,
    pub pre: Vec<Identifier>,
}

impl Range {
    /// `any()` is a factory method which creates a `Range` with no constraints. In other
    /// words, any version will match against it.
    ///
    /// # Examples
    ///
    /// ```
    /// use reproto_semver::Range;
    ///
    /// let anything = Range::any();
    /// ```
    pub fn any() -> Range {
        Range { predicates: vec![] }
    }

    /// `parse()` is the main constructor of a `Range`. It takes a string like `"^1.2.3"`
    /// and turns it into a `Range` that matches that particular constraint.
    ///
    /// A `Result` is returned which contains a `Error` if there was a problem parsing the `Range`.
    ///
    /// # Examples
    ///
    /// ```
    /// use reproto_semver::Range;
    ///
    /// let version = Range::parse("=1.2.3");
    /// let version = Range::parse(">1.2.3");
    /// let version = Range::parse("<1.2.3");
    /// let version = Range::parse("~1.2.3");
    /// let version = Range::parse("^1.2.3");
    /// let version = Range::parse("1.2.3"); // synonym for ^1.2.3
    /// let version = Range::parse("<=1.2.3");
    /// let version = Range::parse(">=1.2.3");
    /// ```
    ///
    /// This example demonstrates error handling, and will panic.
    ///
    /// ```should-panic
    /// use reproto_semver::Range;
    ///
    /// let version = match Range::parse("not a version") {
    ///     Ok(version) => version,
    ///     Err(e) => panic!("There was a problem parsing: {}", e),
    /// }
    /// ```
    pub fn parse(input: &str) -> Result<Range, Error> {
        let mut parser = parser::Parser::new(input)?;
        let range = parser.range()?;

        if !parser.is_eof() {
            return Err(Error::MoreInput);
        }

        Ok(range)
    }

    /// `exact()` is a factory method which creates a `Range` with one exact constraint.
    ///
    /// # Examples
    ///
    /// ```
    /// use reproto_semver::Range;
    /// use reproto_semver::Version;
    ///
    /// let version = Version { major: 1, minor: 1, patch: 1, pre: vec![], build: vec![] };
    /// let exact = Range::exact(&version);
    /// ```
    pub fn exact(version: &Version) -> Range {
        Range {
            predicates: vec![Predicate::exact(version)],
        }
    }

    /// `matches()` matches a given `Version` against this `Range`.
    ///
    /// # Examples
    ///
    /// ```
    /// use reproto_semver::Range;
    /// use reproto_semver::Version;
    ///
    /// let version = Version { major: 1, minor: 1, patch: 1, pre: vec![], build: vec![] };
    /// let exact = Range::exact(&version);
    ///
    /// assert!(exact.matches(&version));
    /// ```
    pub fn matches(&self, version: &Version) -> bool {
        // no predicates means anything matches
        if self.predicates.is_empty() {
            return true;
        }

        self.predicates.iter().all(|p| p.matches(version))
            && self.predicates
                .iter()
                .any(|p| p.pre_tag_is_compatible(version))
    }

    /// Check if range matches any.
    pub fn matches_any(&self) -> bool {
        self.predicates.is_empty()
    }
}

impl Predicate {
    fn exact(version: &Version) -> Predicate {
        Predicate {
            op: Ex,
            major: version.major,
            minor: Some(version.minor),
            patch: Some(version.patch),
            pre: version.pre.clone(),
        }
    }

    /// `matches()` takes a `Version` and determines if it matches this particular `Predicate`.
    pub fn matches(&self, ver: &Version) -> bool {
        match self.op {
            Ex => self.is_exact(ver),
            Gt => self.is_greater(ver),
            GtEq => self.is_exact(ver) || self.is_greater(ver),
            Lt => !self.is_exact(ver) && !self.is_greater(ver),
            LtEq => !self.is_greater(ver),
            Tilde => self.matches_tilde(ver),
            Compatible => self.is_compatible(ver),
            Wildcard(_) => self.matches_wildcard(ver),
        }
    }

    fn is_exact(&self, ver: &Version) -> bool {
        if self.major != ver.major {
            return false;
        }

        match self.minor {
            Some(minor) => {
                if minor != ver.minor {
                    return false;
                }
            }
            None => return true,
        }

        match self.patch {
            Some(patch) => {
                if patch != ver.patch {
                    return false;
                }
            }
            None => return true,
        }

        if self.pre != ver.pre {
            return false;
        }

        true
    }

    // https://docs.npmjs.com/misc/semver#prerelease-tags
    fn pre_tag_is_compatible(&self, ver: &Version) -> bool {
        // If a version has a prerelease tag (for example, 1.2.3-alpha.3) then it will
        // only be
        // allowed to satisfy comparator sets if at least one comparator with the same
        // [major,
        // minor, patch] tuple also has a prerelease tag.
        !ver.is_prerelease()
            || (self.major == ver.major && self.minor == Some(ver.minor)
                && self.patch == Some(ver.patch) && !self.pre.is_empty())
    }

    fn is_greater(&self, ver: &Version) -> bool {
        if self.major != ver.major {
            return ver.major > self.major;
        }

        match self.minor {
            Some(minor) => {
                if minor != ver.minor {
                    return ver.minor > minor;
                }
            }
            None => return false,
        }

        match self.patch {
            Some(patch) => {
                if patch != ver.patch {
                    return ver.patch > patch;
                }
            }
            None => return false,
        }

        if !self.pre.is_empty() {
            return ver.pre.is_empty() || ver.pre > self.pre;
        }

        false
    }

    // see https://www.npmjs.org/doc/misc/semver.html for behavior
    fn matches_tilde(&self, ver: &Version) -> bool {
        let minor = match self.minor {
            Some(n) => n,
            None => return self.major == ver.major,
        };

        match self.patch {
            Some(patch) => {
                self.major == ver.major && minor == ver.minor
                    && (ver.patch > patch || (ver.patch == patch && self.pre_is_compatible(ver)))
            }
            None => self.major == ver.major && minor == ver.minor,
        }
    }

    // see https://www.npmjs.org/doc/misc/semver.html for behavior
    fn is_compatible(&self, ver: &Version) -> bool {
        if self.major != ver.major {
            return false;
        }

        let minor = match self.minor {
            Some(n) => n,
            None => return self.major == ver.major,
        };

        match self.patch {
            Some(patch) => {
                if self.major == 0 {
                    if minor == 0 {
                        ver.minor == minor && ver.patch == patch && self.pre_is_compatible(ver)
                    } else {
                        ver.minor == minor
                            && (ver.patch > patch
                                || (ver.patch == patch && self.pre_is_compatible(ver)))
                    }
                } else {
                    ver.minor > minor
                        || (ver.minor == minor
                            && (ver.patch > patch
                                || (ver.patch == patch && self.pre_is_compatible(ver))))
                }
            }
            None => {
                if self.major == 0 {
                    ver.minor == minor
                } else {
                    ver.minor >= minor
                }
            }
        }
    }

    fn pre_is_compatible(&self, ver: &Version) -> bool {
        ver.pre.is_empty() || ver.pre >= self.pre
    }

    // see https://www.npmjs.org/doc/misc/semver.html for behavior
    fn matches_wildcard(&self, ver: &Version) -> bool {
        match self.op {
            Wildcard(Minor) => self.major == ver.major,
            Wildcard(Patch) => {
                match self.minor {
                    Some(minor) => self.major == ver.major && minor == ver.minor,
                    None => {
                        // minor and patch version astericks mean match on major
                        self.major == ver.major
                    }
                }
            }
            _ => false, // unreachable
        }
    }
}

impl fmt::Display for Range {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        if self.predicates.is_empty() {
            try!(write!(fmt, "*"));
        } else {
            for (i, ref pred) in self.predicates.iter().enumerate() {
                if i == 0 {
                    try!(write!(fmt, "{}", pred));
                } else {
                    try!(write!(fmt, ", {}", pred));
                }
            }
        }

        Ok(())
    }
}

impl fmt::Display for Predicate {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self.op {
            Wildcard(Minor) => try!(write!(fmt, "{}.*", self.major)),
            Wildcard(Patch) => {
                if let Some(minor) = self.minor {
                    try!(write!(fmt, "{}.{}.*", self.major, minor))
                } else {
                    try!(write!(fmt, "{}.*.*", self.major))
                }
            }
            _ => {
                try!(write!(fmt, "{}{}", self.op, self.major));

                match self.minor {
                    Some(v) => try!(write!(fmt, ".{}", v)),
                    None => (),
                }

                match self.patch {
                    Some(v) => try!(write!(fmt, ".{}", v)),
                    None => (),
                }

                if !self.pre.is_empty() {
                    try!(write!(fmt, "-"));
                    for (i, x) in self.pre.iter().enumerate() {
                        if i != 0 {
                            try!(write!(fmt, "."))
                        }
                        try!(write!(fmt, "{}", x));
                    }
                }
            }
        }

        Ok(())
    }
}

impl fmt::Display for Op {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Ex => try!(write!(fmt, "= ")),
            Gt => try!(write!(fmt, "> ")),
            GtEq => try!(write!(fmt, ">= ")),
            Lt => try!(write!(fmt, "< ")),
            LtEq => try!(write!(fmt, "<= ")),
            Tilde => try!(write!(fmt, "~")),
            Compatible => try!(write!(fmt, "^")),
            // gets handled specially in Predicate::fmt
            Wildcard(_) => try!(write!(fmt, "")),
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::super::version::Version;
    use super::{Op, Range};
    use std::hash::{Hash, Hasher};

    fn range(s: &str) -> Range {
        Range::parse(s).unwrap()
    }

    fn version(s: &str) -> Version {
        match Version::parse(s) {
            Ok(v) => v,
            Err(e) => panic!("`{}` is not a valid version. Reason: {:?}", s, e),
        }
    }

    fn assert_match(range: &Range, vers: &[&str]) {
        for ver in vers.iter() {
            assert!(range.matches(&version(*ver)), "did not match {}", ver);
        }
    }

    fn assert_not_match(range: &Range, vers: &[&str]) {
        for ver in vers.iter() {
            assert!(!range.matches(&version(*ver)), "matched {}", ver);
        }
    }

    fn calculate_hash<T: Hash>(t: T) -> u64 {
        use std::collections::hash_map::DefaultHasher;

        let mut s = DefaultHasher::new();
        t.hash(&mut s);
        s.finish()
    }

    #[test]
    fn test_parsing_default() {
        let r = range("1.0.0");

        assert_eq!(r.to_string(), "^1.0.0".to_string());

        assert_match(&r, &["1.0.0", "1.0.1"]);
        assert_not_match(&r, &["0.9.9", "0.10.0", "0.1.0"]);
    }

    #[test]
    fn test_parsing_exact() {
        let r = range("=1.0.0");

        assert!(r.to_string() == "= 1.0.0".to_string());
        assert_eq!(r.to_string(), "= 1.0.0".to_string());

        assert_match(&r, &["1.0.0"]);
        assert_not_match(&r, &["1.0.1", "0.9.9", "0.10.0", "0.1.0", "1.0.0-pre"]);

        let r = range("=0.9.0");

        assert_eq!(r.to_string(), "= 0.9.0".to_string());

        assert_match(&r, &["0.9.0"]);
        assert_not_match(&r, &["0.9.1", "1.9.0", "0.0.9"]);

        let r = range("=0.1.0-beta2.a");

        assert_eq!(r.to_string(), "= 0.1.0-beta2.a".to_string());

        assert_match(&r, &["0.1.0-beta2.a"]);
        assert_not_match(&r, &["0.9.1", "0.1.0", "0.1.1-beta2.a", "0.1.0-beta2"]);
    }

    #[test]
    fn test_parse_metadata_see_issue_88_see_issue_88() {
        for op in &[
            Op::Compatible,
            Op::Ex,
            Op::Gt,
            Op::GtEq,
            Op::Lt,
            Op::LtEq,
            Op::Tilde,
        ] {
            range(&format!("{} 1.2.3+meta", op));
        }
    }

    #[test]
    pub fn test_parsing_greater_than() {
        let r = range(">= 1.0.0");

        assert_eq!(r.to_string(), ">= 1.0.0".to_string());

        assert_match(&r, &["1.0.0", "2.0.0"]);
        assert_not_match(&r, &["0.1.0", "0.0.1", "1.0.0-pre", "2.0.0-pre"]);

        let r = range(">= 2.1.0-alpha2");

        assert_match(&r, &["2.1.0-alpha2", "2.1.0-alpha3", "2.1.0", "3.0.0"]);
        assert_not_match(
            &r,
            &["2.0.0", "2.1.0-alpha1", "2.0.0-alpha2", "3.0.0-alpha2"],
        );
    }

    #[test]
    pub fn test_parsing_less_than() {
        let r = range("< 1.0.0");

        assert_eq!(r.to_string(), "< 1.0.0".to_string());

        assert_match(&r, &["0.1.0", "0.0.1"]);
        assert_not_match(&r, &["1.0.0", "1.0.0-beta", "1.0.1", "0.9.9-alpha"]);

        let r = range("<= 2.1.0-alpha2");

        assert_match(&r, &["2.1.0-alpha2", "2.1.0-alpha1", "2.0.0", "1.0.0"]);
        assert_not_match(
            &r,
            &["2.1.0", "2.2.0-alpha1", "2.0.0-alpha2", "1.0.0-alpha2"],
        );
    }

    #[test]
    pub fn test_multiple() {
        let r = range("> 0.0.9, <= 2.5.3");
        assert_eq!(r.to_string(), "> 0.0.9, <= 2.5.3".to_string());
        assert_match(&r, &["0.0.10", "1.0.0", "2.5.3"]);
        assert_not_match(&r, &["0.0.8", "2.5.4"]);

        let r = range("0.3.0, 0.4.0");
        assert_eq!(r.to_string(), "^0.3.0, ^0.4.0".to_string());
        assert_not_match(&r, &["0.0.8", "0.3.0", "0.4.0"]);

        let r = range("<= 0.2.0, >= 0.5.0");
        assert_eq!(r.to_string(), "<= 0.2.0, >= 0.5.0".to_string());
        assert_not_match(&r, &["0.0.8", "0.3.0", "0.5.1"]);

        let r = range("0.1.0, 0.1.4, 0.1.6");
        assert_eq!(r.to_string(), "^0.1.0, ^0.1.4, ^0.1.6".to_string());
        assert_match(&r, &["0.1.6", "0.1.9"]);
        assert_not_match(&r, &["0.1.0", "0.1.4", "0.2.0"]);

        assert!(Range::parse("> 0.1.0,").is_err());
        assert!(Range::parse("> 0.3.0, ,").is_err());

        let r = range(">=0.5.1-alpha3, <0.6");
        assert_eq!(r.to_string(), ">= 0.5.1-alpha3, < 0.6".to_string());
        assert_match(
            &r,
            &[
                "0.5.1-alpha3",
                "0.5.1-alpha4",
                "0.5.1-beta",
                "0.5.1",
                "0.5.5",
            ],
        );
        assert_not_match(
            &r,
            &["0.5.1-alpha1", "0.5.2-alpha3", "0.5.5-pre", "0.5.0-pre"],
        );
        assert_not_match(&r, &["0.6.0", "0.6.0-pre"]);
    }

    #[test]
    pub fn test_parsing_tilde() {
        let r = range("~1");
        assert_match(&r, &["1.0.0", "1.0.1", "1.1.1"]);
        assert_not_match(&r, &["0.9.1", "2.9.0", "0.0.9"]);

        let r = range("~1.2");
        assert_match(&r, &["1.2.0", "1.2.1"]);
        assert_not_match(&r, &["1.1.1", "1.3.0", "0.0.9"]);

        let r = range("~1.2.2");
        assert_match(&r, &["1.2.2", "1.2.4"]);
        assert_not_match(&r, &["1.2.1", "1.9.0", "1.0.9", "2.0.1", "0.1.3"]);

        let r = range("~1.2.3-beta.2");
        assert_match(&r, &["1.2.3", "1.2.4", "1.2.3-beta.2", "1.2.3-beta.4"]);
        assert_not_match(&r, &["1.3.3", "1.1.4", "1.2.3-beta.1", "1.2.4-beta.2"]);
    }

    #[test]
    pub fn test_parsing_compatible() {
        let r = range("^1");
        assert_match(&r, &["1.1.2", "1.1.0", "1.2.1", "1.0.1"]);
        assert_not_match(&r, &["0.9.1", "2.9.0", "0.1.4"]);
        assert_not_match(&r, &["1.0.0-beta1", "0.1.0-alpha", "1.0.1-pre"]);

        let r = range("^1.1");
        assert_match(&r, &["1.1.2", "1.1.0", "1.2.1"]);
        assert_not_match(&r, &["0.9.1", "2.9.0", "1.0.1", "0.1.4"]);

        let r = range("^1.1.2");
        assert_match(&r, &["1.1.2", "1.1.4", "1.2.1"]);
        assert_not_match(&r, &["0.9.1", "2.9.0", "1.1.1", "0.0.1"]);
        assert_not_match(&r, &["1.1.2-alpha1", "1.1.3-alpha1", "2.9.0-alpha1"]);

        let r = range("^0.1.2");
        assert_match(&r, &["0.1.2", "0.1.4"]);
        assert_not_match(&r, &["0.9.1", "2.9.0", "1.1.1", "0.0.1"]);
        assert_not_match(&r, &["0.1.2-beta", "0.1.3-alpha", "0.2.0-pre"]);

        let r = range("^0.5.1-alpha3");
        assert_match(
            &r,
            &[
                "0.5.1-alpha3",
                "0.5.1-alpha4",
                "0.5.1-beta",
                "0.5.1",
                "0.5.5",
            ],
        );
        assert_not_match(
            &r,
            &[
                "0.5.1-alpha1",
                "0.5.2-alpha3",
                "0.5.5-pre",
                "0.5.0-pre",
                "0.6.0",
            ],
        );

        let r = range("^0.0.2");
        assert_match(&r, &["0.0.2"]);
        assert_not_match(&r, &["0.9.1", "2.9.0", "1.1.1", "0.0.1", "0.1.4"]);

        let r = range("^0.0");
        assert_match(&r, &["0.0.2", "0.0.0"]);
        assert_not_match(&r, &["0.9.1", "2.9.0", "1.1.1", "0.1.4"]);

        let r = range("^0");
        assert_match(&r, &["0.9.1", "0.0.2", "0.0.0"]);
        assert_not_match(&r, &["2.9.0", "1.1.1"]);

        let r = range("^1.4.2-beta.5");
        assert_match(
            &r,
            &["1.4.2", "1.4.3", "1.4.2-beta.5", "1.4.2-beta.6", "1.4.2-c"],
        );
        assert_not_match(
            &r,
            &[
                "0.9.9",
                "2.0.0",
                "1.4.2-alpha",
                "1.4.2-beta.4",
                "1.4.3-beta.5",
            ],
        );
    }

    #[test]
    pub fn test_parsing_wildcard() {
        let r = range("");
        assert_match(&r, &["0.9.1", "2.9.0", "0.0.9", "1.0.1", "1.1.1"]);
        assert_not_match(&r, &[]);
        let r = range("*");
        assert_match(&r, &["0.9.1", "2.9.0", "0.0.9", "1.0.1", "1.1.1"]);
        assert_not_match(&r, &[]);
        let r = range("x");
        assert_match(&r, &["0.9.1", "2.9.0", "0.0.9", "1.0.1", "1.1.1"]);
        assert_not_match(&r, &[]);
        let r = range("X");
        assert_match(&r, &["0.9.1", "2.9.0", "0.0.9", "1.0.1", "1.1.1"]);
        assert_not_match(&r, &[]);

        let r = range("1.*");
        assert_match(&r, &["1.2.0", "1.2.1", "1.1.1", "1.3.0"]);
        assert_not_match(&r, &["0.0.9"]);
        let r = range("1.x");
        assert_match(&r, &["1.2.0", "1.2.1", "1.1.1", "1.3.0"]);
        assert_not_match(&r, &["0.0.9"]);
        let r = range("1.X");
        assert_match(&r, &["1.2.0", "1.2.1", "1.1.1", "1.3.0"]);
        assert_not_match(&r, &["0.0.9"]);

        let r = range("1.2.*");
        assert_match(&r, &["1.2.0", "1.2.2", "1.2.4"]);
        assert_not_match(&r, &["1.9.0", "1.0.9", "2.0.1", "0.1.3"]);
        let r = range("1.2.x");
        assert_match(&r, &["1.2.0", "1.2.2", "1.2.4"]);
        assert_not_match(&r, &["1.9.0", "1.0.9", "2.0.1", "0.1.3"]);
        let r = range("1.2.X");
        assert_match(&r, &["1.2.0", "1.2.2", "1.2.4"]);
        assert_not_match(&r, &["1.9.0", "1.0.9", "2.0.1", "0.1.3"]);
    }

    #[test]
    pub fn test_any() {
        let r = Range::any();
        assert_match(&r, &["0.0.1", "0.1.0", "1.0.0"]);
    }

    #[test]
    pub fn test_pre() {
        let r = range("=2.1.1-really.0");
        assert_match(&r, &["2.1.1-really.0"]);
    }

    #[test]
    pub fn test_from_str() {
        assert_eq!(
            Range::parse("1.0.0").unwrap().to_string(),
            "^1.0.0".to_string()
        );
        assert_eq!(
            Range::parse("=1.0.0").unwrap().to_string(),
            "= 1.0.0".to_string()
        );
        assert_eq!(Range::parse("~1").unwrap().to_string(), "~1".to_string());
        assert_eq!(
            Range::parse("~1.2").unwrap().to_string(),
            "~1.2".to_string()
        );
        assert_eq!(Range::parse("^1").unwrap().to_string(), "^1".to_string());
        assert_eq!(
            Range::parse("^1.1").unwrap().to_string(),
            "^1.1".to_string()
        );
        assert_eq!(Range::parse("*").unwrap().to_string(), "*".to_string());
        assert_eq!(Range::parse("1.*").unwrap().to_string(), "1.*".to_string());
        assert_eq!(
            Range::parse("< 1.0.0").unwrap().to_string(),
            "< 1.0.0".to_string()
        );
    }

    #[test]
    fn test_cargo3202() {
        let v = Range::parse("0.*.*").unwrap();
        assert_eq!("0.*.*", format!("{}", v.predicates[0]));

        let v = Range::parse("0.0.*").unwrap();
        assert_eq!("0.0.*", format!("{}", v.predicates[0]));

        let r = range("0.*.*");
        assert_match(&r, &["0.5.0"]);
    }

    #[test]
    fn test_eq_hash() {
        assert!(range("^1") == range("^1"));
        assert!(calculate_hash(range("^1")) == calculate_hash(range("^1")));
        assert!(range("^1") != range("^2"));
    }

    #[test]
    fn test_ordering() {
        assert!(range("=1") > range("*"));
        assert!(range(">1") > range("*"));
        assert!(range(">=1") > range("*"));
        assert!(range("<1") > range("*"));
        assert!(range("<=1") > range("*"));
        assert!(range("~1") > range("*"));
        assert!(range("^1") > range("*"));
        assert!(range("*") == range("*"));
    }
}
