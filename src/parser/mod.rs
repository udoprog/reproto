pub mod ast;
pub mod errors;
pub mod lexer;
#[allow(unused)]
mod parser;
mod token;
mod utils;

use lalrpop_util::ParseError;
use self::errors::*;
use std::fs;
use std::io::BufReader;
use std::io::Read;
use std::path::Path;

static NL: u8 = '\n' as u8;
static CR: u8 = '\r' as u8;

pub fn find_line(path: &Path, pos: (usize, usize)) -> Result<(String, usize, (usize, usize))> {
    let file = fs::File::open(path)?;
    let reader = BufReader::new(&file);

    let start = pos.0;
    let end = pos.1;

    let mut line_start = 0usize;
    let mut line_buffer: Vec<u8> = Vec::new();
    let mut lines: usize = 0;
    let mut it = reader.bytes().enumerate();

    while let Some((i, b)) = it.next() {
        let b = b?;

        if b == NL || b == CR {
            if i >= start {
                let line = String::from_utf8(line_buffer)?;
                let end = if i > end { end } else { i };
                let range = (start - line_start, end - line_start);
                return Ok((line, lines, range));
            }

            line_start = i;
            lines = lines + 1;
            line_buffer.clear();
            continue;
        }

        line_buffer.push(b);
    }

    Err("bad file position".into())
}

pub fn parse_file(path: &Path) -> Result<ast::File> {
    let mut f = fs::File::open(path)?;
    let mut content = String::new();

    f.read_to_string(&mut content)?;

    let lexer = lexer::lex(content.chars());

    match parser::parse_File(lexer) {
        Ok(file) => Ok(file),
        Err(e) => {
            match e {
                ParseError::InvalidToken { location } => {
                    let pos = (path.to_owned(), location, location);
                    Err(ErrorKind::Syntax(Some(pos), vec![]).into())
                }
                ParseError::UnrecognizedToken { token, expected } => {
                    let pos = token.map(|(start, _, end)| (path.to_owned(), start, end));
                    Err(ErrorKind::Syntax(pos, expected).into())
                }
                ParseError::User { error } => Err(error),
                _ => Err("parse error".into()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::ast::*;

    /// Check that a parsed value equals expected.
    macro_rules! assert_value_eq {
        ($expected:expr, $input:expr) => {{
            let v = parser::parse_Value(parse($input)).unwrap();
            assert_eq!($expected, v);
        }}
    }

    macro_rules! assert_type_spec_eq {
        ($expected:expr, $input:expr) => {{
            let v = parser::parse_TypeSpec(parse($input)).unwrap();
            assert_eq!($expected, v);
        }}
    }

    const FILE1: &[u8] = include_bytes!("tests/file1.reproto");
    const INTERFACE1: &[u8] = include_bytes!("tests/interface1.reproto");

    fn parse(input: &'static str) -> Box<Iterator<Item = Result<(usize, token::Token, usize)>>> {
        Box::new(lexer::lex(input.chars()))
    }

    fn parse_file(input: &'static str) -> File {
        parser::parse_File(parse(input)).unwrap()
    }

    fn parse_member(input: &'static str) -> Member {
        parser::parse_Member(parse(input)).unwrap()
    }

    fn parse_type_spec(input: &'static str) -> RpType {
        parser::parse_TypeSpec(parse(input)).unwrap()
    }

    #[test]
    fn test_file1() {
        let input = ::std::str::from_utf8(FILE1).unwrap();
        let file = parse_file(input);

        let package = RpPackage::new(vec!["foo".to_owned(), "bar".to_owned(), "baz".to_owned()]);

        assert_eq!(package, *file.package);
        assert_eq!(4, file.decls.len());
    }

    #[test]
    fn test_array() {
        let ty = parse_type_spec("[string]");

        if let RpType::Array(inner) = ty {
            if let RpType::String = *inner {
                return;
            }
        }

        panic!("Expected Type::Array(Type::String)");
    }

    #[test]
    fn test_map() {
        let ty = parse_type_spec("{string: unsigned/123}");

        // TODO: use #![feature(box_patterns)]:
        // if let Type::Map(box Type::String, box Type::Unsigned(size)) = ty {
        // }
        if let RpType::Map(key, value) = ty {
            if let RpType::String = *key {
                if let RpType::Unsigned(size) = *value {
                    assert_eq!(Some(123usize), size);
                    return;
                }
            }
        }

        panic!("Expected Type::Array(Type::String)");
    }

    #[test]
    fn test_block_comment() {
        parse("/* hello \n world */");
    }

    #[test]
    fn test_line_comment() {
        parse("// hello world\n");
    }

    #[test]
    fn test_code() {
        parse_member("java{{\na { b { c } d } e\n}}");
    }

    #[test]
    fn test_interface() {
        let input = ::std::str::from_utf8(INTERFACE1).unwrap();
        let file = parse_file(input);
        assert_eq!(1, file.decls.len());
    }

    #[test]
    fn test_instance() {
        let c = RpName {
            prefix: None,
            parts: vec!["Foo".to_owned(), "Bar".to_owned()],
        };

        let field = FieldInit {
            name: AstLoc::new("hello".to_owned(), (8, 13)),
            value: AstLoc::new(Value::Number(12.into()), (15, 17)),
        };

        let field = AstLoc::new(field, (8, 17));

        let instance = Instance {
            ty: c,
            arguments: AstLoc::new(vec![field], (8, 17)),
        };

        assert_value_eq!(Value::Instance(AstLoc::new(instance, (0, 18))),
                         "Foo.Bar(hello: 12)");
    }

    #[test]
    fn test_strings() {
        assert_value_eq!(Value::String("foo\nbar".to_owned()), "\"foo\\nbar\"");
    }

    #[test]
    fn test_numbers() {
        assert_value_eq!(Value::Number(1.into()), "1");

        assert_value_eq!(Value::Number(RpNumber {
                             digits: 125.into(),
                             decimal: 2,
                         }),
                         "1.25");

        assert_value_eq!(Value::Number(RpNumber {
                             digits: 12500.into(),
                             decimal: 0,
                         }),
                         "1.25e4");

        assert_value_eq!(Value::Number(RpNumber {
                             digits: (-12500).into(),
                             decimal: 0,
                         }),
                         "-1.25e4");

        assert_value_eq!(Value::Number(RpNumber {
                             digits: (1234).into(),
                             decimal: 8,
                         }),
                         "0.00001234");
    }

    #[test]
    fn test_type_spec() {
        let c = RpName {
            prefix: None,
            parts: vec!["Hello".to_owned(), "World".to_owned()],
        };

        assert_type_spec_eq!(RpType::String, "string");
        assert_type_spec_eq!(RpType::Name(c), "Hello.World");
    }

    #[test]
    fn test_option_decl() {
        let member = parse_member("foo_bar_baz true, foo, \"bar\", 12;");

        if let Member::Option(option) = member {
            assert_eq!("foo_bar_baz", option.name);
            assert_eq!(4, option.values.len());

            assert_eq!(Value::Boolean(true), option.values[0].inner);
            assert_eq!(Value::Identifier("foo".to_owned()), option.values[1].inner);
            assert_eq!(Value::String("bar".to_owned()), option.values[2].inner);
            assert_eq!(Value::Number(12u32.into()), option.values[3].inner);
            return;
        }

        panic!("option did not match");
    }
}
