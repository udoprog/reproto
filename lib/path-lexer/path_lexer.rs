//! Lexer for paths.

use errors::{Error, Result};
use path_token::PathToken;
use std::str::CharIndices;

pub struct PathLexer<'input> {
    source: CharIndices<'input>,
    source_len: usize,
    source_str: &'input str,
    n0: Option<(usize, char)>,
    n1: Option<(usize, char)>,
    buffer: String,
    segment_start: Option<usize>,
    variable_mode: bool,
}

impl<'input> PathLexer<'input> {
    /// Advance the source iterator.
    #[inline]
    fn step(&mut self) {
        self.n0 = self.n1;
        self.n1 = self.source.next();
    }

    #[inline]
    fn step_n(&mut self, n: usize) -> Option<usize> {
        for _ in 0..n {
            self.step();
        }

        self.n0.map(|v| v.0)
    }

    #[inline]
    fn one(&mut self) -> Option<(usize, char)> {
        self.n0
    }

    #[inline]
    fn two(&mut self) -> Option<(usize, char, char)> {
        if let (Some((pos, a)), Some((_, b))) = (self.n0, self.n1) {
            Some((pos, a, b))
        } else {
            None
        }
    }

    #[inline]
    fn pos(&self) -> usize {
        self.n0.map(|n| n.0).unwrap_or(self.source_len)
    }

    fn token(
        &mut self,
        start: usize,
        token: PathToken<'input>,
    ) -> Result<(usize, PathToken<'input>, usize)> {
        let end = self.step_n(1).unwrap_or(self.source_len);
        Ok((start, token, end))
    }

    fn identifier(&mut self, start: usize) -> Result<(usize, PathToken<'input>, usize)> {
        let (end, content) = take!(self, start, 'a'...'z' | '0'...'9' | '_');
        Ok((start, PathToken::Identifier(content.into()), end))
    }

    fn unexpected(&self) -> Error {
        Error::Unexpected { pos: self.pos() }
    }

    fn normal_mode_next(&mut self) -> Option<Result<(usize, PathToken<'input>, usize)>> {
        // one character keywords
        while let Some((pos, c)) = self.one() {
            let segment_start = match c {
                '{' | '}' | '/' => false,
                '\\' => {
                    match self.two().map(|v| v.2) {
                        Some(c @ '\\') | Some(c @ '/') | Some(c @ '{') | Some(c @ '}') => {
                            self.buffer.push(c)
                        }
                        _ => return Some(Err(self.unexpected())),
                    }

                    self.step_n(2);
                    true
                }
                'a'...'z' if self.variable_mode => {
                    return Some(self.identifier(pos));
                }
                c if !self.variable_mode => {
                    self.buffer.push(c);
                    self.step_n(1);
                    true
                }
                _ => return Some(Err(self.unexpected())),
            };

            if segment_start {
                if self.segment_start.is_none() {
                    self.segment_start = Some(pos);
                }

                continue;
            }

            // return buffered segment.
            if let Some(start) = self.segment_start.take() {
                if !self.buffer.is_empty() {
                    let buffer = self.buffer.clone();
                    self.buffer.clear();
                    return Some(Ok((start, PathToken::Segment(buffer), pos)));
                }
            }

            let out = match c {
                '{' if !self.variable_mode => {
                    self.variable_mode = true;
                    self.token(pos, PathToken::LeftCurly)
                }
                '}' if self.variable_mode => {
                    self.variable_mode = false;
                    self.token(pos, PathToken::RightCurly)
                }
                '/' => self.token(pos, PathToken::Slash),
                _ => return Some(Err(self.unexpected())),
            };

            return Some(out);
        }

        if let Some(start) = self.segment_start.take() {
            if !self.buffer.is_empty() {
                let buffer = self.buffer.clone();
                return Some(Ok((start, PathToken::Segment(buffer), self.source_len)));
            }
        }

        None
    }
}

impl<'input> Iterator for PathLexer<'input> {
    type Item = Result<(usize, PathToken<'input>, usize)>;

    fn next(&mut self) -> Option<Self::Item> {
        self.normal_mode_next()
    }
}

pub fn path_lex(input: &str) -> PathLexer {
    let mut source = input.char_indices();

    let n0 = source.next();
    let n1 = source.next();

    PathLexer {
        source: source,
        source_len: input.len(),
        source_str: input,
        n0: n0,
        n1: n1,
        buffer: String::new(),
        segment_start: None,
        variable_mode: false,
    }
}

#[cfg(test)]
pub mod tests {
    use super::PathToken::*;
    use super::*;

    fn tokenize(input: &str) -> Result<Vec<(usize, PathToken, usize)>> {
        path_lex(input).collect()
    }

    #[test]
    pub fn test_path_lexer() {
        let input = "foo/\\{bar/\\/baz/{id}\\/\\\\\\{\\}";

        let expected = vec![
            (0, Segment("foo".to_string()), 3),
            (3, Slash, 4),
            (4, Segment("{bar".to_string()), 9),
            (9, Slash, 10),
            (10, Segment("/baz".to_string()), 15),
            (15, Slash, 16),
            (16, LeftCurly, 17),
            (17, Identifier("id".into()), 19),
            (19, RightCurly, 20),
            (20, Segment("/\\{}".to_string()), 28),
        ];

        assert_eq!("{id}", &input[16..20]);
        assert_eq!(expected, tokenize(input).unwrap());
    }

    #[test]
    pub fn test_path_err() {
        let input = " \\id";
        let expected = Err(Error::Unexpected { pos: 1 });
        assert_eq!(expected, tokenize(input));
    }
}
