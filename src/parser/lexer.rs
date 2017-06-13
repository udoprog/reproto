use core::RpNumber;
use num::Zero;
use num::bigint::BigInt;
use super::errors::*;
use super::token::*;

macro_rules! take {
    ($slf:expr, $first:pat $(| $rest:pat)*) => {{
        let mut end = None;
        $slf.buffer.clear();

        while let Some((p, c)) = $slf.one() {
            if let Some((_, '/', '*')) = $slf.two() {
                $slf.block_comment();
                continue;
            }

            match c {
                $first $(| $rest)* => {
                    end = Some(p);
                    $slf.buffer.push(c);
                },
                _ => break,
            }

            $slf.step();
        }

        (end, &$slf.buffer)
    }}
}

pub struct Lexer<I> {
    source: I,
    pos: usize,
    n0: Option<Option<char>>,
    n1: Option<Option<char>>,
    last_comment: Vec<String>,
    buffer: String,
    illegal: bool,
    code_block: bool,
}

impl<I> Lexer<I>
    where I: Iterator<Item = char>
{
    /// Advance the source iterator.
    #[inline]
    fn step(&mut self) {
        if self.n0.map(|n| n.is_some()).unwrap_or(false) {
            self.pos += 1;
        }

        // shift
        if let Some(n1) = self.n1 {
            self.n0 = Some(n1);
            self.n1 = Some(self.source.next());
        } else {
            self.n0 = Some(self.source.next());
            self.n1 = Some(self.source.next());
        }
    }

    #[inline]
    fn one(&mut self) -> Option<(usize, char)> {
        if self.n0.is_none() {
            self.step();
        }

        match self.n0 {
            Some(n0) => n0.map(|n| (self.pos, n)),
            None => None,
        }
    }

    #[inline]
    fn two(&mut self) -> Option<(usize, char, char)> {
        if self.n0.is_none() {
            self.step();
        }

        if let (Some(a), Some(b)) = (self.n0.unwrap_or(None), self.n1.unwrap_or(None)) {
            Some((self.pos, a, b))
        } else {
            None
        }
    }

    fn identifier(&mut self, start: usize) -> Result<(usize, Token, usize)> {
        let (end, content) = take!(self, 'a'...'z' | '_' | '0'...'9');
        let end = end.unwrap_or(start);

        let token = match content.as_str() {
            "any" => Token::AnyKeyword,
            "interface" => Token::InterfaceKeyword,
            "type" => Token::TypeKeyword,
            "enum" => Token::EnumKeyword,
            "tuple" => Token::TupleKeyword,
            "package" => Token::PackageKeyword,
            "match" => Token::MatchKeyword,
            "use" => Token::UseKeyword,
            "as" => Token::AsKeyword,
            "float" => Token::FloatKeyword,
            "double" => Token::DoubleKeyword,
            "signed" => Token::SignedKeyword,
            "unsigned" => Token::UnsignedKeyword,
            "boolean" => Token::BooleanKeyword,
            "string" => Token::StringKeyword,
            "bytes" => Token::BytesKeyword,
            "true" => Token::TrueKeyword,
            "false" => Token::FalseKeyword,
            identifier => {
                Token::Identifier(Commented::new(self.last_comment.clone(), identifier.to_owned()))
            }
        };

        self.last_comment.clear();
        return Ok((start, token, end + 1));
    }

    fn type_identifier(&mut self, start: usize) -> Result<(usize, Token, usize)> {
        let (end, content) = take!(self, 'A'...'Z' | 'a'...'z' | '_' | '0'...'9');
        let end = end.unwrap_or(start);
        self.last_comment.clear();
        Ok((start, Token::TypeIdentifier(content.to_owned()), end + 1))
    }

    fn parse_fraction(input: &str) -> Result<(usize, BigInt)> {
        let dec = input.chars()
            .enumerate()
            .find(|&(_, ref c)| *c != '0')
            .map(|(i, _)| i)
            .unwrap_or(0usize);

        let fraction: BigInt = input.parse()?;

        Ok((dec, fraction))
    }

    fn apply_fraction(digits: &mut BigInt, decimal: &mut usize, dec: usize, fraction: BigInt) {
        *decimal += dec;

        let mut f = fraction.clone();
        let ten: BigInt = 10.into();

        while !f.is_zero() {
            *digits = digits.clone() * ten.clone();
            *decimal += 1;
            f = f / ten.clone();
        }

        *digits = digits.clone() + fraction;
    }

    fn apply_exponent(digits: &mut BigInt, decimal: &mut usize, exponent: i32) {
        if exponent < 0 {
            *decimal += exponent.abs() as usize;
            return;
        }

        let ten: BigInt = 10.into();

        for _ in 0..exponent {
            if *decimal > 0 {
                *decimal = *decimal - 1;
            } else {
                *digits = digits.clone() * ten.clone();
            }
        }
    }

    fn number(&mut self, start: usize) -> Result<(usize, Token, usize)> {
        let negative = if let Some((_, '-')) = self.one() {
            self.step();
            true
        } else {
            false
        };

        let (mut end, mut digits) = {
            let (e, whole) = take!(self, '0'...'9');
            (e.unwrap_or(start), whole.parse::<BigInt>()?)
        };

        let mut decimal = 0usize;

        if let Some((_, '.')) = self.one() {
            self.step();

            {
                let (e, fraction) = take!(self, '0'...'9');
                let (dec, fraction) = Self::parse_fraction(fraction)?;
                Self::apply_fraction(&mut digits, &mut decimal, dec, fraction);
                end = e.unwrap_or(end);
            }

            if let Some((_, 'e')) = self.one() {
                self.step();

                let (e, content) = take!(self, '-' | '0'...'9');
                let exponent: i32 = content.parse()?;
                Self::apply_exponent(&mut digits, &mut decimal, exponent);
                end = e.unwrap_or(end);
            }
        }

        let digits = if negative { -digits } else { digits };

        let number = RpNumber {
            digits: digits,
            decimal: decimal,
        };

        self.last_comment.clear();
        Ok((start, Token::Number(number), end + 1))
    }

    fn decode_unicode4(&mut self) -> Result<char> {
        let mut res = 0u32;

        for x in 0..4u32 {
            let c = self.one().map(|(_, c)| c).ok_or("expected hex character")?.to_string();
            let c = u32::from_str_radix(&c, 16)?;
            res += c << (4 * (3 - x));
            self.step();
        }

        Ok(::std::char::from_u32(res).ok_or("expected valid character")?)
    }

    /// Tokenize string.
    fn string(&mut self, start: usize) -> Result<(usize, Token, usize)> {
        self.buffer.clear();

        self.step();

        while let Some((p, c)) = self.one() {
            if c == '\\' {
                self.step();

                if let Some((_, escape)) = self.one() {
                    let escaped = match escape {
                        'n' => '\n',
                        'r' => '\r',
                        't' => '\t',
                        'u' => self.decode_unicode4()?,
                        _ => break,
                    };

                    self.step();
                    self.buffer.push(escaped);
                    continue;
                }

                break;
            }

            if c == '"' {
                self.step();
                self.last_comment.clear();
                return Ok((start, Token::String(self.buffer.clone()), p + 1));
            }

            self.buffer.push(c);
            self.step();
        }

        Err(ErrorKind::IllegalToken.into())
    }

    /// Tokenize code block.
    /// TODO: support escape sequences for languages where `}}` might occur.
    fn code_block(&mut self) -> Result<(usize, Token, usize)> {
        self.buffer.clear();
        let mut start = None;

        while let Some((p, c)) = self.one() {
            if start.is_none() {
                start = Some(p);
            }

            if let Some((p, '}', '}')) = self.two() {
                self.code_block = false;
                let start = start.unwrap_or(p);
                return Ok((start, Token::CodeContent(self.buffer.clone()), p));
            }

            self.buffer.push(c);
            self.step();
        }

        Err(ErrorKind::IllegalToken.into())
    }

    fn line_comment(&mut self) {
        self.buffer.clear();

        self.step();
        self.step();

        while let Some((_, c)) = self.one() {
            match c {
                '\n' | '\r' => {
                    self.step();
                    break;
                }
                c => {
                    self.buffer.push(c);
                    self.step();
                }
            }
        }

        self.last_comment.push(self.buffer.clone());
    }

    // block comments have no semantics and are completely ignored.
    fn block_comment(&mut self) {
        self.step();
        self.step();

        while let Some((_, a, b)) = self.two() {
            if ('*', '/') == (a, b) {
                self.step();
                self.step();
                break;
            }

            self.step();
        }
    }
}

impl<I> Iterator for Lexer<I>
    where I: Iterator<Item = char>
{
    type Item = Result<(usize, Token, usize)>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.illegal {
            return Some(Err(ErrorKind::IllegalToken.into()));
        }

        // code block mode
        if self.code_block {
            return Some(self.code_block());
        }

        loop {
            // two character keywords
            if let Some((start, a, b)) = self.two() {
                let token = match (a, b) {
                    ('/', '/') => {
                        self.line_comment();
                        continue;
                    }
                    ('/', '*') => {
                        self.block_comment();
                        continue;
                    }
                    ('}', '}') => Some(Token::CodeClose),
                    ('{', '{') => {
                        self.code_block = true;
                        Some(Token::CodeOpen)
                    }
                    (':', ':') => Some(Token::Scope),
                    ('=', '>') => Some(Token::HashRocket),
                    _ => None,
                };

                if let Some(token) = token {
                    self.step();
                    self.step();
                    self.last_comment.clear();
                    return Some(Ok((start, token, start + 2)));
                }
            }

            // one character keywords
            if let Some((start, c)) = self.one() {
                let token = match c {
                    '{' => Token::LeftCurly,
                    '}' => Token::RightCurly,
                    '[' => Token::LeftBracket,
                    ']' => Token::RightBracket,
                    '(' => Token::LeftParen,
                    ')' => Token::RightParen,
                    ';' => Token::SemiColon,
                    ':' => Token::Colon,
                    ',' => Token::Comma,
                    '.' => Token::Dot,
                    '?' => Token::Optional,
                    '/' => Token::Slash,
                    '=' => Token::Equals,
                    'a'...'z' => return Some(self.identifier(start)),
                    'A'...'Z' => return Some(self.type_identifier(start)),
                    '"' => return Some(self.string(start)),
                    '-' | '0'...'9' => return Some(self.number(start)),
                    // ignore whitespace
                    ' ' | '\n' | '\r' | '\t' => {
                        self.step();
                        continue;
                    }
                    _ => break,
                };

                self.last_comment.clear();
                self.step();
                return Some(Ok((start, token, start + 1)));
            } else {
                return None;
            }
        }

        self.illegal = true;
        Some(Err(ErrorKind::IllegalToken.into()))
    }
}

pub fn lex<I>(input: I) -> Lexer<I>
    where I: Iterator<Item = char>
{
    Lexer {
        source: input,
        pos: 0usize,
        n0: None,
        n1: None,
        last_comment: Vec::new(),
        buffer: String::new(),
        illegal: false,
        code_block: false,
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use super::Token::*;

    fn tokenize(input: &str) -> Result<Vec<(usize, Token, usize)>> {
        lex(input.chars()).collect()
    }

    #[test]
    pub fn test_lexer() {
        let expected = vec![(0, Identifier(Commented::empty("hello".into())), 5),
                            (6, TypeIdentifier("World".into()), 10),
                            (12, LeftCurly, 13),
                            (14, UseKeyword, 17),
                            (18, AsKeyword, 20),
                            (21, RightCurly, 22),
                            (23, String("hello world".into()), 36)];

        assert_eq!(expected,
                   tokenize("hello World { use as } \"hello world\"").unwrap());
    }

    #[test]
    pub fn test_code_block() {
        let expected = vec![(0, CodeOpen, 2),
                            (2, CodeContent(" foo bar baz \n zing ".into()), 22),
                            (22, CodeClose, 24)];

        assert_eq!(expected, tokenize("{{ foo bar baz \n zing }}").unwrap());
    }

    #[test]
    pub fn test_complex_number() {
        let expected = vec![(0,
                             Number(RpNumber {
                                 digits: (-1242).into(),
                                 decimal: 6,
                             }),
                             9)];

        assert_eq!(expected, tokenize("-12.42e-4").unwrap());
    }

    #[test]
    pub fn test_number_2() {
        assert_eq!(vec![(0, Number(12.into()), 2)], tokenize("12").unwrap());
    }

    #[test]
    pub fn test_name() {
        let expected = vec![(0, Identifier(Commented::empty("foo".into())), 3),
                            (3, Scope, 5),
                            (5, TypeIdentifier("Bar".into()), 7),
                            (8, Dot, 9),
                            (9, TypeIdentifier("Baz".into()), 11)];

        assert_eq!(expected, tokenize("foo::Bar.Baz").unwrap());
    }

    #[test]
    pub fn test_instance() {
        let expected = vec![(0, Identifier(Commented::empty("foo".into())), 3),
                            (3, Scope, 5),
                            (5, TypeIdentifier("Bar".into()), 7),
                            (8, Dot, 9),
                            (9, TypeIdentifier("Baz".into()), 11),
                            (12, LeftParen, 13),
                            (13, Identifier(Commented::empty("hello".into())), 18),
                            (18, Colon, 19),
                            (20, Number(12.into()), 22),
                            (22, RightParen, 23)];

        assert_eq!(expected, tokenize("foo::Bar.Baz(hello: 12)").unwrap());
    }

    #[test]
    pub fn test_comments() {
        let tokens = tokenize("// hello \n world");
        let comment = vec![" hello ".into()];
        assert_eq!(vec![(11, Identifier(Commented::new(comment, "world".into())), 16)],
                   tokens.unwrap());

        let tokens = tokenize("he/* this is a comment */llo");
        assert_eq!(vec![(0, Identifier(Commented::empty("hello".into())), 28)],
                   tokens.unwrap());

        let tokens = tokenize("// test\n// this\nhello");
        let comment = vec![" test".into(), " this".into()];

        assert_eq!(vec![(16, Identifier(Commented::new(comment, "hello".into())), 21)],
                   tokens.unwrap());
    }
}
