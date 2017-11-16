use core::{RpNumber, VersionReq};

#[derive(Debug)]
pub enum Error {
    UnterminatedString { start: usize },
    UnterminatedEscape { start: usize },
    InvalidEscape { message: &'static str, pos: usize },
    UnterminatedCodeBlock { start: usize },
    InvalidNumber { message: &'static str, pos: usize },
    Unexpected { pos: usize },
    InvalidVersionReq { start: usize, end: usize },
}

pub type Result<T> = ::std::result::Result<T, Error>;

#[derive(Clone, Debug, PartialEq)]
pub enum Token<'input> {
    Identifier(&'input str),
    TypeIdentifier(&'input str),
    DocComment(Vec<&'input str>),
    Number(RpNumber),
    VersionReq(VersionReq),
    LeftCurly,
    RightCurly,
    LeftBracket,
    RightBracket,
    LeftParen,
    RightParen,
    SemiColon,
    Colon,
    Comma,
    Dot,
    Scope,
    QuestionMark,
    Slash,
    RightArrow,
    CodeOpen,
    CodeClose,
    CodeContent(&'input str),
    String(String),
    // identifier-style keywords
    InterfaceKeyword,
    TypeKeyword,
    EnumKeyword,
    TupleKeyword,
    ServiceKeyword,
    UseKeyword,
    AsKeyword,
    AnyKeyword,
    FloatKeyword,
    DoubleKeyword,
    SignedKeyword,
    UnsignedKeyword,
    BooleanKeyword,
    StringKeyword,
    DateTimeKeyword,
    BytesKeyword,
    TrueKeyword,
    FalseKeyword,
    StreamKeyword,
    Tick,
    At,
    PathSegment(String),
}
