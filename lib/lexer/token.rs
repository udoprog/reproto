use core::{ContentSlice, RpNumber};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Token<S> {
    Identifier(S),
    TypeIdentifier(S),
    PackageDocComment(Vec<S>),
    DocComment(Vec<S>),
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
    CodeContent(S),
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

impl<S> Token<S>
where
    S: ContentSlice,
{
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

    pub fn as_ident<'a>(&'a self) -> Option<&'a S>
    where
        &'a S: From<&'static str>,
    {
        use self::Token::*;

        let ident = match *self {
            Any => "any".into(),
            Interface => "interface".into(),
            Type => "type".into(),
            Enum => "enum".into(),
            Tuple => "tuple".into(),
            Service => "service".into(),
            Use => "use".into(),
            As => "as".into(),
            Float => "float".into(),
            Double => "double".into(),
            I32 => "i32".into(),
            I64 => "i64".into(),
            U32 => "u32".into(),
            U64 => "u64".into(),
            Boolean => "boolean".into(),
            String => "string".into(),
            Datetime => "datetime".into(),
            Bytes => "bytes".into(),
            Stream => "stream".into(),
            Identifier(ref ident) => ident,
            _ => return None,
        };

        Some(ident)
    }
}
