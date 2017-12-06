/// Lexer for paths.

use errors::Error::*;
use errors::Result;
use path_token::PathToken;
use std::str::CharIndices;

pub struct PathLexer<'input> {
    source: CharIndices<'input>,
    source_len: usize,
    source_str: &'input str,
    n0: Option<(usize, char)>,
    buffer: String,
    segment_start: Option<usize>,
    escape: bool,
}

impl<'input> PathLexer<'input> {
    /// Advance the source iterator.
    #[inline]
    fn step(&mut self) {
        self.n0 = self.source.next();
    }

    #[inline]
    fn step_n(&mut self, n: usize) -> usize {
        for _ in 0..n {
            self.step();
        }

        self.n0.map(|n| n.0).unwrap_or_else(
            || self.source_str.len(),
        )
    }

    #[inline]
    fn one(&mut self) -> Option<(usize, char)> {
        self.n0
    }

    #[inline]
    fn pos(&self) -> usize {
        self.n0.map(|n| n.0).unwrap_or(self.source_len)
    }

    fn normal_mode_next(&mut self) -> Option<Result<(usize, PathToken<'input>, usize)>> {
        // one character keywords
        while let Some((pos, c)) = self.one() {
            if self.escape {
                match c {
                    '/' | '{' => {
                        self.buffer.push(c);
                    }
                    _ => break,
                }

                self.step_n(1);
                self.escape = false;
                continue;
            }

            let token = match c {
                '{' => {
                    let start = self.step_n(1);
                    let (end, content) = take_until!(self, start, '}');
                    return Some(Ok((pos, PathToken::Variable(content), end)));
                }
                '/' => Some(PathToken::Slash),
                '\\' => {
                    self.escape = true;
                    None
                }
                c => {
                    self.buffer.push(c);
                    None
                }
            };

            if let Some(token) = token {
                if let Some(start) = self.segment_start.take() {
                    if !self.buffer.is_empty() {
                        let buffer = self.buffer.clone();
                        self.buffer.clear();
                        return Some(Ok((start, PathToken::Segment(buffer), pos)));
                    }
                }

                let end = self.step_n(1);
                return Some(Ok((pos, token, end)));
            }

            if self.segment_start.is_none() {
                self.segment_start = Some(pos);
            }

            self.step_n(1);
        }

        if let Some(start) = self.segment_start.take() {
            if !self.buffer.is_empty() {
                let buffer = self.buffer.clone();
                return Some(Ok((start, PathToken::Segment(buffer), self.source_len)));
            }
        }

        if self.escape {
            return Some(Err(Unexpected { pos: self.pos() }));
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

    PathLexer {
        source: source,
        source_len: input.len(),
        source_str: input,
        n0: n0,
        buffer: String::new(),
        segment_start: None,
        escape: false,
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use super::PathToken::*;

    fn tokenize(input: &str) -> Result<Vec<(usize, PathToken, usize)>> {
        path_lex(input).collect()
    }

    #[test]
    pub fn test_path_lexer() {
        let input = "foo/\\{bar/\\/baz/{id}";

        let expected = vec![
            (0, Segment("foo".to_string()), 3),
            (3, Slash, 4),
            (4, Segment("{bar".to_string()), 9),
            (9, Slash, 10),
            (10, Segment("/baz".to_string()), 15),
            (15, Slash, 16),
            (16, Variable("id"), 20),
        ];

        assert_eq!("{id}", &input[16..20]);
        assert_eq!(expected, tokenize(input).unwrap());
    }
}
