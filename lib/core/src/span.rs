#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn empty() -> Span {
        Span { start: 0, end: 0 }
    }
}

impl<'a> From<&'a Span> for Span {
    fn from(value: &'a Span) -> Span {
        Span {
            start: value.start,
            end: value.end,
        }
    }
}

impl From<(usize, usize)> for Span {
    fn from(value: (usize, usize)) -> Self {
        Span {
            start: value.0,
            end: value.1,
        }
    }
}
