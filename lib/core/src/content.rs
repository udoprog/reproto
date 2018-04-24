use std::fmt;
use std::ops;

pub trait ContentSlice
where
    Self: Sized,
    Self: Clone + Copy + fmt::Debug + From<&'static str>,
{
    /// Iterator over lines.
    type Lines: DoubleEndedIterator<Item = Self>;
    type Chars: Iterator<Item = char>;

    /// Iterate over lines.
    fn lines(self) -> Self::Lines;

    /// Iterate over characters.
    fn chars(self) -> Self::Chars;

    /// Get length of slice.
    fn len(self) -> usize;

    /// Create a sub-slice.
    fn slice(self, range: ops::Range<usize>) -> Self;

    /// Create a sub-slice.
    fn slice_from(self, range: ops::RangeFrom<usize>) -> Self;

    /// Convert to string.
    fn to_string(self) -> String;

    /// Find the first index that matches the given predicate.
    fn find<'a, P>(&'a self, pat: P) -> Option<usize>
    where
        P: Fn(char) -> bool;
}

/// A contiguous source of content.
pub trait Content {
    type Slice: ContentSlice;

    /// Get the next character and corresponding index from the content.
    fn next(&mut self) -> Option<(usize, char)>;

    /// Get a slice.
    fn slice(&self, start: usize, end: usize) -> Self::Slice;

    /// Get the length of the content.
    fn len(&self) -> usize;
}
