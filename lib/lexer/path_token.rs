#[derive(Clone, Debug, PartialEq)]
pub enum PathToken<'input> {
    /// Variable capture.
    Variable(&'input str),
    /// Potentially escaped segment.
    Segment(String),
    Slash,
}
