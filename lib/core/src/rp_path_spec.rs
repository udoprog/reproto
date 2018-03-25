//! Path specifications

use errors::Result;
use std::vec;
use {Flavor, RpEndpointArgument, Translate, Translator};

/// A part of a step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RpPathPart<F: 'static>
where
    F: Flavor,
{
    Variable(RpEndpointArgument<F>),
    Segment(String),
}

impl<F: 'static, T> Translate<T> for RpPathPart<F>
where
    F: Flavor,
    T: Translator<Source = F>,
{
    type Source = F;
    type Out = RpPathPart<T::Target>;

    /// Translate into different flavor.
    fn translate(self, translator: &T) -> Result<RpPathPart<T::Target>> {
        use self::RpPathPart::*;

        let out = match self {
            Variable(arg) => Variable(arg.translate(translator)?),
            Segment(segment) => Segment(segment),
        };

        Ok(out)
    }
}

/// A step in a path specification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RpPathStep<F: 'static>
where
    F: Flavor,
{
    pub parts: Vec<RpPathPart<F>>,
}

impl<F: 'static, T> Translate<T> for RpPathStep<F>
where
    F: Flavor,
    T: Translator<Source = F>,
{
    type Source = F;
    type Out = RpPathStep<T::Target>;

    /// Translate into different flavor.
    fn translate(self, translator: &T) -> Result<RpPathStep<T::Target>> {
        Ok(RpPathStep {
            parts: self.parts.translate(translator)?,
        })
    }
}

/// A path specification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RpPathSpec<F: 'static>
where
    F: Flavor,
{
    pub steps: Vec<RpPathStep<F>>,
}

#[derive(Debug)]
pub struct Vars<'a, F: 'static>
where
    F: Flavor,
{
    iter: vec::IntoIter<&'a RpEndpointArgument<F>>,
}

impl<'a, F: 'static> Iterator for Vars<'a, F>
where
    F: Flavor,
{
    type Item = &'a RpEndpointArgument<F>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl<F: 'static> RpPathSpec<F>
where
    F: Flavor,
{
    /// List all variables in the path spec.
    pub fn vars(&self) -> Vars<F> {
        let mut vars = Vec::new();

        for step in &self.steps {
            for part in &step.parts {
                if let RpPathPart::Variable(ref var) = *part {
                    vars.push(var);
                }
            }
        }

        Vars {
            iter: vars.into_iter(),
        }
    }
}

impl<F: 'static, T> Translate<T> for RpPathSpec<F>
where
    F: Flavor,
    T: Translator<Source = F>,
{
    type Source = F;
    type Out = RpPathSpec<T::Target>;

    /// Translate into different flavor.
    fn translate(self, translator: &T) -> Result<RpPathSpec<T::Target>> {
        Ok(RpPathSpec {
            steps: self.steps.translate(translator)?,
        })
    }
}
