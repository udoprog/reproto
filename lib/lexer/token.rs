use core::RpNumber;
use std::borrow::Cow;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Token<'input> {
    Identifier(Cow<'input, str>),
    TypeIdentifier(Cow<'input, str>),
    PackageDocComment(Vec<Cow<'input, str>>),
    DocComment(Vec<Cow<'input, str>>),
    Number(RpNumber),
    LeftCurly,
    RightCurly,
    LeftBracket,
    RightBracket,
    LeftParen,
    RightParen,
    SemiColon,
    Colon,
    Equal,
    Comma,
    Dot,
    Scope,
    QuestionMark,
    Hash,
    Bang,
    RightArrow,
    CodeOpen,
    CodeClose,
    CodeContent(Cow<'input, str>),
    QuotedString(String),
    // identifier-style keywords
    Any,
    As,
    Boolean,
    Bytes,
    Datetime,
    Enum,
    Float,
    Double,
    I32,
    I64,
    Interface,
    Service,
    Stream,
    String,
    Tuple,
    Type,
    U32,
    U64,
    Use,
}

impl<'input> Token<'input> {
    /// Get the keywords-safe variant of the given keyword.
    pub fn keyword_safe(&self) -> Option<&'static str> {
        use self::Token::*;

        let out = match *self {
            Any => "_any",
            As => "_as",
            Boolean => "_boolean",
            Bytes => "_bytes",
            Datetime => "_datetime",
            Enum => "_enum",
            Float => "_float",
            Double => "_double",
            I32 => "_i32",
            I64 => "_i64",
            Interface => "_interface",
            Service => "_service",
            Stream => "_stream",
            String => "_string",
            Tuple => "_tuple",
            Type => "_type",
            U32 => "_u32",
            U64 => "_u64",
            Use => "_use",
            _ => return None,
        };

        Some(out)
    }

    pub fn as_ident(&self) -> Option<&str> {
        use self::Token::*;

        let ident = match *self {
            Any => "any",
            Interface => "interface",
            Type => "type",
            Enum => "enum",
            Tuple => "tuple",
            Service => "service",
            Use => "use",
            As => "as",
            Float => "float",
            Double => "double",
            I32 => "i32",
            I64 => "i64",
            U32 => "u32",
            U64 => "u64",
            Boolean => "boolean",
            String => "string",
            Datetime => "datetime",
            Bytes => "bytes",
            Stream => "stream",
            Identifier(ref ident) => ident.as_ref(),
            _ => return None,
        };

        Some(ident)
    }
}
