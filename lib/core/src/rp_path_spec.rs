//! Path specifications

use RpEndpointArgument;
use std::rc::Rc;
use std::vec;

/// A part of a step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RpPathPart {
    Variable(Rc<RpEndpointArgument>),
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
    iter: vec::IntoIter<&'a RpEndpointArgument>,
}

impl<'a> Iterator for Vars<'a> {
    type Item = &'a RpEndpointArgument;

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
                    vars.push(Rc::as_ref(var));
                }
            }
        }

        Vars {
            iter: vars.into_iter(),
        }
    }
}
