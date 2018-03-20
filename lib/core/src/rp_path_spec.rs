//! Path specifications

use std::vec;

/// A part of a step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RpPathPart {
    Variable(String),
    Segment(String),
}

/// A step in a path specification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RpPathStep {
    pub parts: Vec<RpPathPart>,
}

/// A path specification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RpPathSpec {
    pub steps: Vec<RpPathStep>,
}

#[derive(Debug)]
pub struct Vars<'a> {
    iter: vec::IntoIter<&'a str>,
}

impl<'a> Iterator for Vars<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl RpPathSpec {
    /// List all variables in the path spec.
    pub fn vars(&self) -> Vars {
        let mut vars = Vec::new();

        for step in &self.steps {
            for part in &step.parts {
                if let RpPathPart::Variable(ref var) = *part {
                    vars.push(var.as_str());
                }
            }
        }

        Vars { iter: vars.into_iter() }
    }
}
