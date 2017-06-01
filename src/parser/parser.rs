#![allow(unconditional_recursion)]

use backend::models as m;
use pest::prelude::*;
use std::collections::LinkedList;
use super::ast;
use super::errors::*;

/// Check if character is an indentation character.
fn is_indent(c: char) -> bool {
    match c {
        ' ' | '\t' => true,
        _ => false,
    }
}

/// Find the number of whitespace characters that the given string is indented.
fn find_indent(input: &str) -> Option<usize> {
    input.chars().enumerate().find(|&(_, c)| !is_indent(c)).map(|(i, _)| i)
}

fn code_block_indent(input: &str) -> Option<(usize, usize, usize)> {
    let mut indent: Option<usize> = None;

    let mut start = 0;
    let mut end = 0;

    let mut first_line = false;

    for (line_no, line) in input.lines().enumerate() {
        if let Some(current) = find_indent(line) {
            end = line_no + 1;

            if indent.map(|i| i > current).unwrap_or(true) {
                indent = Some(current);
            }

            first_line = true;
        } else {
            if !first_line {
                start += 1;
            }
        }
    }

    indent.map(|indent| (indent, start, end - start))
}

/// Strip common indent from all input lines.
fn strip_code_block(input: &str) -> Vec<String> {
    if let Some((indent, empty_start, len)) = code_block_indent(input) {
        input.lines()
            .skip(empty_start)
            .take(len)
            .map(|line| {
                if line.len() < indent {
                    line.to_owned()
                } else {
                    (&line[indent..]).to_owned()
                }
            })
            .collect()
    } else {
        input.lines().map(ToOwned::to_owned).collect()
    }
}

/// Decode an escaped string.
fn decode_escaped_string(input: &str) -> Result<String> {
    let mut out = String::new();
    let mut it = input.chars().skip(1).peekable();

    loop {
        let c = match it.next() {
            None => break,
            Some(c) => c,
        };

        // strip end quote
        if it.peek().is_none() {
            break;
        }

        if c == '\\' {
            let escaped = match it.next().ok_or("expected character")? {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                'u' => decode_unicode4(&mut it)?,
                _ => return Err(ErrorKind::InvalidEscape.into()),
            };

            out.push(escaped);
            continue;
        }

        out.push(c);
    }

    Ok(out)
}

/// Decode the next four characters as a unicode escape sequence.
fn decode_unicode4(it: &mut Iterator<Item = char>) -> Result<char> {
    let mut res = 0u32;

    for x in 0..4u32 {
        let c = it.next().ok_or("expected hex character")?.to_string();
        let c = u32::from_str_radix(&c, 16)?;
        res += c << (4 * (3 - x));
    }

    Ok(::std::char::from_u32(res).ok_or("expected valid character")?)
}

impl_rdp! {
    grammar! {
        file = _{ package_decl ~ use_decl* ~ decl* ~ eoi }
        decl = { type_decl | interface_decl | tuple_decl | enum_decl }

        use_decl = { use_keyword ~ package_ident ~ use_as? ~ semi_colon }
        use_as = { as_keyword ~ identifier }

        package_decl = { package_keyword ~ package_ident ~ semi_colon }

        type_decl = { type_keyword ~ type_identifier ~ left_curly ~ type_body ~ right_curly }
        type_body = _{ member* }

        tuple_decl = { tuple_keyword ~ type_identifier ~ left_curly ~ tuple_body ~ right_curly }
        tuple_body = _{ member* }

        interface_decl = { interface_keyword ~ type_identifier ~ left_curly ~ interface_body ~ right_curly }
        interface_body = _{ member* ~ sub_type* }

        enum_decl = { enum_keyword ~ type_identifier ~ left_curly ~ enum_body ~ right_curly }
        enum_body = _{ enum_body_value* ~ member* }
        enum_body_value = { enum_value }

        sub_type = { type_identifier ~ left_curly ~ sub_type_body ~ right_curly }
        sub_type_body = _{ member* }

        member = { option_decl | match_decl | field | code_block }
        field = { identifier ~ optional? ~ colon ~ type_spec ~ field_as? ~ semi_colon }
        field_as = { as_keyword ~ value }
        code_block = @{ identifier ~ whitespace* ~ code_start ~ code_body ~ code_end }
        code_body = { (!(["}}"]) ~ any)* }

        enum_value = { type_identifier ~ enum_arguments? ~ enum_ordinal? ~ semi_colon }
        enum_arguments = { (left_paren ~ (value ~ (comma ~ value)*) ~ right_paren) }
        enum_ordinal = { equals ~ value }
        option_decl = { identifier ~ (value ~ (comma ~ value)*) ~ semi_colon }

        match_decl = { match_keyword ~ left_curly ~ match_member_entry* ~ right_curly }
        match_member_entry = { match_member }
        match_member = { match_condition ~ hash_rocket ~ value ~ semi_colon }
        match_condition = { match_variable | match_value }
        match_variable = { identifier ~ colon ~ type_spec }
        match_value = { value }

        package_ident = @{ identifier ~ (dot ~ identifier)* }

        type_spec = _{
            float_type |
            double_type |
            signed_type |
            unsigned_type |
            boolean_type |
            string_type |
            bytes_type |
            any_type |
            map_type |
            array_type |
            custom_type
        }

        // Types
        float_type = @{ ["float"] }
        double_type = @{ ["double"] }
        signed_type = @{ ["signed"] ~ type_bits? }
        unsigned_type = @{ ["unsigned"] ~ type_bits? }
        boolean_type = @{ ["boolean"] }
        string_type = @{ ["string"] }
        bytes_type = @{ ["bytes"] }
        any_type = @{ ["any"] }
        map_type = { left_curly ~ type_spec ~ colon ~ type_spec ~ right_curly }
        array_type = { ["["] ~ type_spec ~ ["]"] }
        custom_type = @{ used_prefix? ~ type_identifier ~ (dot ~ type_identifier)* }

        used_prefix = @{ identifier ~ scope }

        // Keywords and tokens
        enum_keyword = @{ ["enum"] }
        use_keyword = @{ ["use"] }
        as_keyword = @{ ["as"] }
        package_keyword = @{ ["package"] }
        type_keyword = @{ ["type"] }
        tuple_keyword = @{ ["tuple"] }
        interface_keyword = @{ ["interface"] }
        match_keyword = @{ ["match"] }
        hash_rocket = @{ ["=>"] }
        comma = @{ [","] }
        colon = @{ [":"] }
        scope = @{ ["::"] }
        semi_colon = @{ [";"] }
        left_curly = @{ ["{"] }
        right_curly = @{ ["}"] }
        code_start = @{ ["{{"] }
        code_end = @{ ["}}"] }
        left_paren = @{ ["("] }
        right_paren = @{ [")"] }
        forward_slash = @{ ["/"] }
        optional = @{ ["?"] }
        equals = @{ ["="] }
        dot = @{ ["."] }

        type_bits = _{ (forward_slash ~ unsigned) }

        value = { instance | constant | boolean | identifier | string | number }

        instance = { custom_type ~ (left_paren ~ (field_init ~ (comma ~ field_init)*)? ~ right_paren) }
        constant = { custom_type }

        field_init = { identifier ~ colon ~ value }

        identifier = @{ ['a'..'z'] ~ (['0'..'9'] | ['a'..'z'] | ["_"])* }
        type_identifier = @{ ['A'..'Z'] ~ (['A'..'Z'] | ['a'..'z'])* }

        string  = @{ ["\""] ~ (escape | !(["\""] | ["\\"]) ~ any)* ~ ["\""] }
        escape  =  _{ ["\\"] ~ (["\""] | ["\\"] | ["/"] | ["n"] | ["r"] | ["t"] | unicode) }
        unicode =  _{ ["u"] ~ hex ~ hex ~ hex ~ hex }
        hex     =  _{ ['0'..'9'] | ['a'..'f'] }

        unsigned = @{ int }
        number   = @{ ["-"]? ~ int ~ (dot ~ ['0'..'9']+)? ~ (["e"] ~ int)? }
        int      =  _{ ["0"] | ['1'..'9'] ~ ['0'..'9']* }

        boolean = { ["true"] | ["false"] }

        whitespace = _{ [" "] | ["\t"] | ["\r"] | ["\n"] }

        comment = _{
            // line comment
            ( ["//"] ~ (!(["\r"] | ["\n"]) ~ any)* ~ (["\n"] | ["\r\n"] | ["\r"] | eoi) ) |
            // block comment
            ( ["/*"] ~ (!(["*/"]) ~ any)* ~ ["*/"] )
        }
    }

    process! {
        _file(&self) -> Result<ast::File> {
            (
                _: package_decl,
                _: package_keyword,
                package: _package(), _: semi_colon,
                uses: _use_list(),
                decls: _decl_list(),
            ) => {
                let package = package;
                let uses = uses?.into_iter().collect();
                let decls = decls?.into_iter().collect();

                Ok(ast::File {
                    package: package,
                    uses: uses,
                    decls: decls
                })
            },
        }

        _use_list(&self) -> Result<LinkedList<ast::Token<ast::UseDecl>>> {
            (token: use_decl, use_decl: _use_decl(), tail: _use_list()) => {
                let pos = (token.start, token.end);
                let mut tail = tail?;
                tail.push_front(ast::Token::new(use_decl, pos));
                Ok(tail)
            },

            () => Ok(LinkedList::new()),
        }

        _use_decl(&self) -> ast::UseDecl {
            (_: use_keyword, package: _package(), alias: _use_as(), _: semi_colon) => {
                ast::UseDecl {
                    package: package,
                    alias: alias,
                }
            }
        }

        _use_as(&self) -> Option<String> {
            (_: use_as, _: as_keyword, &alias: identifier) => Some(alias.to_owned()),
            () => None,
        }

        _package(&self) -> ast::Token<m::Package> {
            (token: package_ident, idents: _ident_list()) => {
                let pos = (token.start, token.end);
                let idents = idents;
                let package = m::Package::new(idents.into_iter().collect());
                ast::Token::new(package, pos)
            },
        }

        _decl_list(&self) -> Result<LinkedList<ast::Token<ast::Decl>>> {
            (token: decl, value: _decl(), tail: _decl_list()) => {
                let mut tail = tail?;
                let pos = (token.start, token.end);
                tail.push_front(ast::Token::new(value?, pos));
                Ok(tail)
            },

            () => Ok(LinkedList::new()),
        }

        _decl(&self) -> Result<ast::Decl> {
            (
                _: type_decl,
                _: type_keyword,
                &name: type_identifier,
                _: left_curly,
                members: _member_list(),
                _: right_curly,
            ) => {
                let members = members?.into_iter().collect();

                let body = ast::TypeBody {
                    name: name.to_owned(),
                    members: members
                };

                Ok(ast::Decl::Type(body))
            },

            (
                _: tuple_decl,
                _: tuple_keyword,
                &name: type_identifier,
                _: left_curly,
                members: _member_list(),
                _: right_curly,
            ) => {
                let members = members?.into_iter().collect();

                let body = ast::TupleBody {
                    name: name.to_owned(),
                    members: members,
                };

                Ok(ast::Decl::Tuple(body))
            },

            (
                _: interface_decl,
                _: interface_keyword,
                &name: type_identifier,
                _: left_curly,
                members: _member_list(),
                sub_types: _sub_type_list(),
                _: right_curly,
            ) => {
                let members = members?.into_iter().collect();
                let sub_types = sub_types?.into_iter().collect();

                let body = ast::InterfaceBody {
                    name: name.to_owned(),
                    members: members,
                    sub_types: sub_types,
                };

                Ok(ast::Decl::Interface(body))
            },

            (
                _: enum_decl,
                _: enum_keyword,
                &name: type_identifier,
                _: left_curly,
                values: _enum_value_list(),
                members: _member_list(),
                _: right_curly,
            ) => {
                let values = values?.into_iter().collect();
                let members = members?.into_iter().collect();

                let body = ast::EnumBody {
                    name: name.to_owned(),
                    values: values,
                    members: members,
                };

                Ok(ast::Decl::Enum(body))
            },
        }

        _enum_value_list(&self) -> Result<LinkedList<ast::Token<ast::EnumValue>>> {
            (_: enum_body_value, value: _enum_value(), tail: _enum_value_list()) => {
                let mut tail = tail?;
                tail.push_front(value?);
                Ok(tail)
            },

            () => Ok(LinkedList::new()),
        }

        _enum_value(&self) -> Result<ast::Token<ast::EnumValue>> {
            (
                token: enum_value,
                &name: type_identifier,
                values: _enum_arguments(),
                ordinal: _enum_ordinal(),
                _: semi_colon
             ) => {
                let arguments = values?.into_iter().collect();
                let ordinal = ordinal?;
                let pos = (token.start, token.end);
                let enum_value = ast::EnumValue { name: name.to_owned(), arguments: arguments, ordinal: ordinal };
                Ok(ast::Token::new(enum_value, pos))
            },
        }

        _enum_arguments(&self) -> Result<LinkedList<ast::Token<ast::Value>>> {
            (_: enum_arguments, _: left_paren, values: _value_list(), _: right_paren) => values,
            () => Ok(LinkedList::new()),
        }

        _enum_ordinal(&self) -> Result<Option<ast::Token<ast::Value>>> {
            (_: enum_ordinal, _: equals, value: _value_token()) => value.map(Some),
            () => Ok(None),
        }

        _value_list(&self) -> Result<LinkedList<ast::Token<ast::Value>>> {
            (value: _value_token(), _: comma, tail: _value_list()) => {
                let mut tail = tail?;
                tail.push_front(value?);
                Ok(tail)
            },

            (value: _value_token()) => {
                let mut tail = LinkedList::new();
                tail.push_front(value?);
                Ok(tail)
            },
        }

        _value_token(&self) -> Result<ast::Token<ast::Value>> {
            (token: value, value: _value()) => {
                let pos = (token.start, token.end);
                value.map(move |v| ast::Token::new(v, pos))
            },
        }

        _value(&self) -> Result<ast::Value> {
            (
                token: instance,
                _: custom_type,
                custom: _custom(),
                _: left_paren,
                arguments: _field_init_list(),
                _: right_paren,
            ) => {
                let pos = (token.start, token.end);
                let arguments = arguments?.into_iter().collect();

                let instance = ast::Instance {
                   ty: custom,
                   arguments: arguments,
                };

                Ok(ast::Value::Instance(ast::Token::new(instance, pos)))
            },

            (
                token: constant,
                _: custom_type,
                custom: _custom(),
            ) => {
                let pos = (token.start, token.end);

                let instance = ast::Constant {
                    prefix: custom.prefix,
                    parts: custom.parts,
                };

                Ok(ast::Value::Constant(ast::Token::new(instance, pos)))
            },

            (&value: string) => {
                let value = decode_escaped_string(value)?;
                Ok(ast::Value::String(value))
            },

            (&value: identifier) => {
                Ok(ast::Value::Identifier(value.to_owned()))
            },

            (&value: number) => {
                let value = value.parse::<f64>()?;
                Ok(ast::Value::Number(value))
            },

            (&value: boolean) => {
                let value = match value {
                    "true" => true,
                    "false" => false,
                    _ => panic!("should not happen"),
                };

                Ok(ast::Value::Boolean(value))
            },
        }

        _used_prefix(&self) -> Option<String> {
            (_: used_prefix, &prefix: identifier, _: scope) => Some(prefix.to_owned()),
            () => None,
        }

        _field_init_list(&self) -> Result<LinkedList<ast::Token<ast::FieldInit>>> {
            (
                token: field_init,
                &name: identifier,
                _: colon,
                value: _value_token(),
                _: comma,
                tail: _field_init_list()
            ) => {
                let mut tail = tail?;
                let pos = (token.start, token.end);
                let name = name.to_owned();
                let value = value?;

                let field_init = ast::FieldInit {
                    name: ast::Token::new(name, pos),
                    value: value,
                };

                tail.push_front(ast::Token::new(field_init, pos));
                Ok(tail)
            },

            (
                token: field_init,
                &name: identifier,
                _: colon,
                value: _value_token(),
                tail: _field_init_list()
            ) => {
                let mut tail = tail?;
                let pos = (token.start, token.end);
                let name = name.to_owned();
                let value = value?;

                let field_init = ast::FieldInit {
                    name: ast::Token::new(name, pos),
                    value: value,
                };

                tail.push_front(ast::Token::new(field_init, pos));
                Ok(tail)
            },

            () => Ok(LinkedList::new()),
        }

        _member_list(&self) -> Result<LinkedList<ast::Token<ast::Member>>> {
            (token: member, value: _member(), tail: _member_list()) => {
                let mut tail = tail?;
                let pos = (token.start, token.end);
                tail.push_front(ast::Token::new(value?, pos));
                Ok(tail)
            },

            () => Ok(LinkedList::new()),
        }

        _member(&self) -> Result<ast::Member> {
            (
                _: field,
                &name: identifier,
                modifier: _modifier(),
                _: colon,
                type_spec: _type_spec(),
                field_as: _field_as(),
                _: semi_colon,
            ) => {
                let field = ast::Field {
                    modifier: modifier,
                    name: name.to_owned(),
                    ty: type_spec?,
                    field_as: field_as?,
                };

                Ok(ast::Member::Field(field))
            },

            (
                _: code_block,
                &context: identifier,
                _: code_start,
                &content: code_body,
                _: code_end,
             ) => {
                let block = strip_code_block(content);
                Ok(ast::Member::Code(context.to_owned(), block))
            },

            (
                token: option_decl,
                &name: identifier,
                values: _value_list(),
                _: semi_colon,
            ) => {
                let pos = (token.start, token.end);
                let values = values?.into_iter().collect();
                let option_decl = ast::OptionDecl { name: name.to_owned(), values: values };
                Ok(ast::Member::Option(ast::Token::new(option_decl, pos)))
            },

            (
                _: match_decl,
                _: match_keyword,
                _: left_curly,
                members: _match_member_list(),
                _: right_curly,
             ) => {
                let members = members?.into_iter().collect();

                let decl = ast::MatchDecl {
                    members: members,
                };

                Ok(ast::Member::Match(decl))
            },
        }

        _field_as(&self) -> Result<Option<ast::Token<ast::Value>>> {
            (_: field_as, _: as_keyword, value: _value_token()) => Ok(Some(value?)),
            () => Ok(None),
        }

        _sub_type_list(&self) -> Result<LinkedList<ast::Token<ast::SubType>>> {
            (token: sub_type, value: _sub_type(), tail: _sub_type_list()) => {
                let mut tail = tail?;
                let pos = (token.start, token.end);
                tail.push_front(ast::Token::new(value?, pos));
                Ok(tail)
            },

            () => {
                Ok(LinkedList::new())
            },
        }

        _sub_type(&self) -> Result<ast::SubType> {
            (
                &name: type_identifier,
                _: left_curly,
                members: _member_list(),
                _: right_curly,
             ) => {
                let name = name.to_owned();
                let members = members?.into_iter().collect();
                Ok(ast::SubType { name: name, members: members })
            },
        }

        _match_member_list(&self) -> Result<LinkedList<ast::Token<ast::MatchMember>>> {
            (
                _: match_member_entry,
                member: _match_member(),
                tail: _match_member_list(),
            ) => {
                let mut tail = tail?;
                tail.push_front(member?);
                Ok(tail)
            },

            () => Ok(LinkedList::new()),
        }

        _match_member(&self) -> Result<ast::Token<ast::MatchMember>> {
            (
                token: match_member,
                condition: _match_condition(),
                _: hash_rocket,
                value: _value_token(),
                _: semi_colon,
            ) => {
                let pos = (token.start, token.end);

                let member = ast::MatchMember {
                    condition: condition?,
                    value: value?,
                };

                Ok(ast::Token::new(member, pos))
            },
        }

        _match_condition(&self) -> Result<ast::Token<ast::MatchCondition>> {
            (
                token: match_condition,
                _: match_value,
                value: _value_token(),
            ) => {
                let pos = (token.start, token.end);
                let value = value?;
                let condition = ast::MatchCondition::Value(value);
                Ok(ast::Token::new(condition, pos))
            },

            (
                token: match_condition,
                _: match_variable,
                &name: identifier,
                _: colon,
                ty: _type_spec(),
            ) => {
                let pos = (token.start, token.end);
                let name = name.to_owned();
                let ty = ty?;

                let variable = ast::MatchVariable {
                    name: name,
                    ty: ty,
                };

                let condition = ast::MatchCondition::Type(variable);

                Ok(ast::Token::new(condition, pos))
            },
        }

        _type_spec(&self) -> Result<m::Type> {
            (_: double_type) => {
                Ok(m::Type::Double)
            },

            (_: float_type) => {
                Ok(m::Type::Float)
            },

            (_: signed_type, _: forward_slash, &size: unsigned) => {
                let size = size.parse::<usize>()?;
                Ok(m::Type::Signed(Some(size)))
            },

            (_: unsigned_type, _: forward_slash, &size: unsigned) => {
                let size = size.parse::<usize>()?;
                Ok(m::Type::Unsigned(Some(size)))
            },

            (_: signed_type) => {
                Ok(m::Type::Signed(None))
            },

            (_: unsigned_type) => {
                Ok(m::Type::Unsigned(None))
            },

            (_: boolean_type) => {
                Ok(m::Type::Boolean)
            },

            (_: string_type) => {
                Ok(m::Type::String)
            },

            (_: bytes_type) => {
                Ok(m::Type::Bytes)
            },

            (_: any_type) => {
                Ok(m::Type::Any)
            },

            (_: array_type, argument: _type_spec()) => {
                let argument = argument?;
                Ok(m::Type::Array(Box::new(argument)))
            },

            (
                _: map_type,
                _: left_curly,
                key: _type_spec(),
                _: colon,
                value: _type_spec(),
                _: right_curly
             ) => {
                let key = key?;
                let value = value?;
                Ok(m::Type::Map(Box::new(key), Box::new(value)))
            },

            (_: custom_type, custom: _custom()) => {
                Ok(m::Type::Custom(custom))
            },
        }

        _custom(&self) -> m::Custom {
            (prefix: _used_prefix(), parts: _type_identifier_list()) => {
                let parts = parts.into_iter().collect();

                m::Custom {
                    prefix: prefix,
                    parts: parts,
                }
            },
        }

        _modifier(&self) -> m::Modifier {
            (_: optional) => m::Modifier::Optional,
            () => m::Modifier::Required,
        }

        _ident_list(&self) -> LinkedList<String> {
            (&value: identifier, _: dot, mut tail: _ident_list()) => {
                tail.push_front(value.to_owned());
                tail
            },

            (&value: identifier, mut tail: _ident_list()) => {
                tail.push_front(value.to_owned());
                tail
            },

            () => LinkedList::new(),
        }

        _type_identifier_list(&self) -> LinkedList<String> {
            (&value: type_identifier, _: dot, mut tail: _type_identifier_list()) => {
                tail.push_front(value.to_owned());
                tail
            },

            (&value: type_identifier) => {
                let mut tail = LinkedList::new();
                tail.push_front(value.to_owned());
                tail
            },

        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Check that a parsed value equals expected.
    macro_rules! assert_value_eq {
        ($expected:expr, $input:expr) => {{
            let mut parser = parse($input);
            assert!(parser.value());

            let v = parser._value_token().unwrap().inner;
            assert_eq!($expected, v);
        }}
    }

    macro_rules! assert_type_spec_eq {
        ($expected:expr, $input:expr) => {{
            let mut parser = parse($input);
            assert!(parser.type_spec());
            assert!(parser.end());

            let v = parser._type_spec().unwrap();
            assert_eq!($expected, v);
        }}
    }

    const FILE1: &[u8] = include_bytes!("tests/file1.reproto");
    const INTERFACE1: &[u8] = include_bytes!("tests/interface1.reproto");

    fn parse(input: &'static str) -> Rdp<StringInput> {
        Rdp::new(StringInput::new(input))
    }

    #[test]
    fn test_file1() {
        let input = ::std::str::from_utf8(FILE1).unwrap();
        let mut parser = parse(input);

        assert!(parser.file());
        assert!(parser.end());

        let file = parser._file().unwrap();

        let package = m::Package::new(vec!["foo".to_owned(), "bar".to_owned(), "baz".to_owned()]);

        assert_eq!(package, *file.package);
        assert_eq!(4, file.decls.len());
    }

    #[test]
    fn test_array() {
        let mut parser = parse("[string]");

        assert!(parser.type_spec());
        assert!(parser.end());

        let ty = parser._type_spec().unwrap();

        if let m::Type::Array(inner) = ty {
            if let m::Type::String = *inner {
                return;
            }
        }

        panic!("Expected Type::Array(Type::String)");
    }

    #[test]
    fn test_map() {
        let mut parser = parse("{string: unsigned/123}");

        assert!(parser.type_spec());
        assert!(parser.end());

        let ty = parser._type_spec().unwrap();

        // TODO: use #![feature(box_patterns)]:
        // if let m::Type::Map(box m::Type::String, box m::Type::Unsigned(size)) = ty {
        // }
        if let m::Type::Map(key, value) = ty {
            if let m::Type::String = *key {
                if let m::Type::Unsigned(size) = *value {
                    assert_eq!(Some(123usize), size);
                    return;
                }
            }
        }

        panic!("Expected Type::Array(Type::String)");
    }

    #[test]
    fn test_block_comment() {
        let mut parser = parse("/* hello \n world */");

        assert!(parser.comment());
    }

    #[test]
    fn test_line_comment() {
        let mut parser = parse("// hello world\n");

        assert!(parser.comment());
    }

    #[test]
    fn test_code_block() {
        let mut parser = parse("a { b { c } d } e");

        assert!(parser.code_body());
        assert!(parser.end());
    }

    #[test]
    fn test_code() {
        let mut parser = parse("java{{\na { b { c } d } e\n}}");

        assert!(parser.code_block());
        assert!(parser.end());
    }

    #[test]
    fn test_find_indent() {
        assert_eq!(Some(4), find_indent("   \thello"));
        assert_eq!(Some(0), find_indent("nope"));
        assert_eq!(None, find_indent(""));
        assert_eq!(None, find_indent("    "));
    }

    #[test]
    fn test_strip_code_block() {
        let result = strip_code_block("\n   hello\n  world\n\n\n again\n\n\n");
        assert_eq!(vec!["  hello", " world", "", "", "again"], result);
    }

    #[test]
    fn test_interface() {
        let input = ::std::str::from_utf8(INTERFACE1).unwrap();
        let mut parser = parse(input);

        assert!(parser.file());
        assert!(parser.end());

        let file = parser._file().unwrap();

        assert_eq!(1, file.decls.len());
    }

    #[test]
    fn test_values() {
        let field = ast::FieldInit {
            name: ast::Token::new("hello".to_owned(), (8, 17)),
            value: ast::Token::new(ast::Value::Number(12f64), (15, 17)),
        };

        let field = ast::Token::new(field, (8, 17));

        let instance = ast::Instance {
            ty: m::Type::Custom(None, vec!["Foo".to_owned(), "Bar".to_owned()]),
            arguments: vec![field],
        };

        assert_value_eq!(ast::Value::Instance(ast::Token::new(instance, (0, 18))),
                         "Foo.Bar(hello: 12)");
        assert_value_eq!(ast::Value::String("foo\nbar".to_owned()), "\"foo\\nbar\"");
        assert_value_eq!(ast::Value::Number(1f64), "1");
    }

    #[test]
    fn test_type_spec() {
        assert_type_spec_eq!(m::Type::String, "string");
        assert_type_spec_eq!(m::Type::Custom(None, vec!["Hello".to_owned(), "World".to_owned()]),
                             "Hello.World");
    }

    #[test]
    fn test_option_decl() {
        let mut parser = parse("foo_bar_baz true, foo, \"bar\", 12;");

        assert!(parser.option_decl());
        assert!(parser.end());

        if let ast::Member::Option(option) = parser._member().unwrap() {
            assert_eq!("foo_bar_baz", option.name);
            assert_eq!(4, option.values.len());

            assert_eq!(ast::Value::Boolean(true), option.values[0].inner);
            assert_eq!(ast::Value::Identifier("foo".to_owned()),
                       option.values[1].inner);
            assert_eq!(ast::Value::String("bar".to_owned()), option.values[2].inner);
            assert_eq!(ast::Value::Number(12f64), option.values[3].inner);
            return;
        }

        panic!("option did not match");
    }
}
