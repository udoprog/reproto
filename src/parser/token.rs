use core::RpNumber;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commented<T> {
    pub comment: Vec<String>,
    pub value: T,
}

impl<T> Commented<T> {
    pub fn new(comment: Vec<String>, value: T) -> Commented<T> {
        Commented {
            comment: comment,
            value: value,
        }
    }

    pub fn empty(value: T) -> Commented<T> {
        Commented {
            comment: vec![],
            value: value,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Token {
    Identifier(Commented<String>),
    TypeIdentifier(String),
    Number(RpNumber),
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
    Optional,
    Slash,
    Equals,
    HashRocket,
    CodeOpen,
    CodeClose,
    CodeContent(String),
    String(String),
    // identifier-style keywords
    InterfaceKeyword,
    TypeKeyword,
    EnumKeyword,
    TupleKeyword,
    PackageKeyword,
    MatchKeyword,
    UseKeyword,
    AsKeyword,
    AnyKeyword,
    FloatKeyword,
    DoubleKeyword,
    SignedKeyword,
    UnsignedKeyword,
    BooleanKeyword,
    StringKeyword,
    BytesKeyword,
    TrueKeyword,
    FalseKeyword,
}
