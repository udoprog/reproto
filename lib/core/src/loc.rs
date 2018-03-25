use serde;
use std::borrow;
use std::cmp;
use std::hash;
use std::result;
use {Pos, WithPos};

#[derive(Clone)]
pub struct Loc<T> {
    inner: T,
    pos: Pos,
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
    pub fn new<P: Into<Pos>>(inner: T, pos: P) -> Loc<T> {
        Loc {
            inner: inner,
            pos: pos.into(),
        }
    }

    pub fn value(loc: &Loc<T>) -> &T {
        &loc.inner
    }

    pub fn pos(loc: &Loc<T>) -> &Pos {
        &loc.pos
    }

    pub fn take(loc: Loc<T>) -> T {
        loc.inner
    }

    pub fn take_pair(loc: Loc<T>) -> (T, Pos) {
        (loc.inner, loc.pos)
    }

    pub fn borrow_pair(loc: &Loc<T>) -> (&T, &Pos) {
        (&loc.inner, &loc.pos)
    }

    pub fn map<U, O>(loc: Loc<T>, op: O) -> Loc<U>
    where
        O: FnOnce(T) -> U,
    {
        Loc::new(op(loc.inner), loc.pos.clone())
    }

    pub fn and_then<U, O, E: WithPos>(
        Loc { inner, pos }: Loc<T>,
        op: O,
    ) -> result::Result<Loc<U>, E>
    where
        O: FnOnce(T) -> result::Result<U, E>,
    {
        op(inner)
            .map(|value| Loc::new(value, pos.clone()))
            .with_pos(&pos)
    }

    pub fn as_ref(loc: &Loc<T>) -> Loc<&T> {
        Loc::new(&loc.inner, loc.pos.clone())
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
        write!(f, "<{:?}@{:?}>", self.inner, self.pos)
    }
}

impl<'a, T> From<&'a Loc<T>> for Pos {
    fn from(value: &'a Loc<T>) -> Self {
        Loc::pos(value).clone()
    }
}
