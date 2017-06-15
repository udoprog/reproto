use serde;
use std::path::PathBuf;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Loc<T, P> {
    pub inner: T,
    pub pos: P,
}

impl<T, P> serde::Serialize for Loc<T, P>
    where T: serde::Serialize,
          P: serde::Serialize
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: serde::Serializer
    {
        (&self.inner, &self.pos).serialize(serializer)
    }
}

impl<T, P> ::std::ops::Deref for Loc<T, P> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.inner
    }
}

impl<T, P> Loc<T, P>
    where P: Clone
{
    pub fn new(inner: T, pos: P) -> Loc<T, P> {
        Loc {
            inner: inner,
            pos: pos,
        }
    }

    pub fn map_inner<M, U>(self, map: M) -> Loc<U, P>
        where M: FnOnce(T) -> U
    {
        Loc::new(map(self.inner), self.pos)
    }
}

pub trait WithPrefix<T> {
    type Prefix;
    type Output;

    fn with_prefix(self, prefix: Self::Prefix) -> Loc<T, Self::Output>;
}

impl<T, B, C> WithPrefix<T> for Loc<T, (B, C)>
    where T: Clone,
          B: Clone,
          C: Clone
{
    type Prefix = PathBuf;
    type Output = (PathBuf, B, C);

    fn with_prefix(self, prefix: Self::Prefix) -> Loc<T, Self::Output> {
        let (b, c) = self.pos.clone();
        Loc::new(self.inner.clone(), (prefix, b, c))
    }
}

impl<T, P> ::std::fmt::Display for Loc<T, P>
    where T: ::std::fmt::Display
{
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl<T, P> ::std::fmt::Debug for Loc<T, P>
    where T: ::std::fmt::Debug,
          P: ::std::fmt::Debug
{
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "<{:?}@{:?}>", self.inner, self.pos)
    }
}
