use Span;
use serde;
use std::borrow;
use std::cmp;
use std::hash;
use std::result;

#[derive(Clone)]
pub struct Loc<T> {
    inner: T,
    span: Span,
}

impl<T: serde::Serialize> serde::Serialize for Loc<T> {
    fn serialize<S>(&self, serializer: S) -> result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.inner.serialize(serializer)
    }
}

impl<T> Loc<T> {
    pub fn new<P: Into<Span>>(inner: T, span: P) -> Loc<T> {
        Loc {
            inner: inner,
            span: span.into(),
        }
    }

    pub fn span(loc: &Loc<T>) -> Span {
        loc.span
    }

    pub fn take(loc: Loc<T>) -> T {
        loc.inner
    }

    pub fn take_pair(loc: Loc<T>) -> (T, Span) {
        (loc.inner, loc.span)
    }

    pub fn borrow(loc: &Loc<T>) -> &T {
        &loc.inner
    }

    pub fn borrow_pair(loc: &Loc<T>) -> (&T, Span) {
        (&loc.inner, loc.span)
    }

    pub fn map<U, O>(loc: Loc<T>, op: O) -> Loc<U>
    where
        O: FnOnce(T) -> U,
    {
        Loc::new(op(loc.inner), loc.span.clone())
    }

    pub fn as_ref(loc: &Loc<T>) -> Loc<&T> {
        Loc::new(&loc.inner, loc.span.clone())
    }

    /// Apply the fallible operation over the given location.
    pub fn and_then<U, O, E>(Loc { inner, span }: Loc<T>, op: O) -> result::Result<Loc<U>, E>
    where
        O: FnOnce(T) -> result::Result<U, E>,
    {
        op(inner).map(|value| Loc::new(value, span.clone()))
    }
}

impl<T> cmp::PartialEq for Loc<T>
where
    T: cmp::PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.inner.eq(other)
    }
}

impl<T> cmp::Eq for Loc<T>
where
    T: cmp::Eq,
{
}

impl<T> cmp::PartialOrd for Loc<T>
where
    T: cmp::PartialOrd,
{
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        self.inner.partial_cmp(other)
    }
}

impl<T> cmp::Ord for Loc<T>
where
    T: cmp::Ord,
    Self: cmp::PartialOrd + cmp::Eq,
{
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.inner.cmp(other)
    }
}

impl<T> hash::Hash for Loc<T>
where
    T: hash::Hash,
{
    fn hash<H>(&self, state: &mut H)
    where
        H: hash::Hasher,
    {
        self.inner.hash(state)
    }
}

impl<T> ::std::ops::Deref for Loc<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.inner
    }
}

impl<T> ::std::ops::DerefMut for Loc<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<T> ::std::convert::AsMut<T> for Loc<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<T> borrow::Borrow<T> for Loc<T> {
    fn borrow(&self) -> &T {
        &**self
    }
}

impl<T> ::std::fmt::Display for Loc<T>
where
    T: ::std::fmt::Display,
{
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl<T> ::std::fmt::Debug for Loc<T>
where
    T: ::std::fmt::Debug,
{
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "<{:?}@{:?}>", self.inner, self.span)
    }
}

impl<'a, T> From<&'a Loc<T>> for Span {
    fn from(value: &'a Loc<T>) -> Self {
        Loc::span(value).clone()
    }
}
