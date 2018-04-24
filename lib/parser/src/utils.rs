//! Utility functions for the parser.

use core::ContentSlice;

/// Check if character is not an indentation character.
fn is_not_indent(c: char) -> bool {
    match c {
        ' ' | '\t' => false,
        _ => true,
    }
}

/// Strip common indent from all input lines.
pub fn strip_code_block<'a, S>(input: S) -> Vec<S>
where
    S: ContentSlice,
{
    let num_empty_start = input
        .lines()
        .take_while(|line| line.chars().all(char::is_whitespace))
        .count();

    let num_empty_end = input
        .lines()
        .rev()
        .take_while(|line| line.chars().all(char::is_whitespace))
        .count();

    let indent = input
        .lines()
        .flat_map(|line| line.find(is_not_indent).into_iter())
        .min();

    let mut it = input.lines();

    // strip empty lines from the front
    for _ in 0..num_empty_start {
        it.next();
    }

    // strip empty lines from the tail
    for _ in 0..num_empty_end {
        it.next_back();
    }

    if let Some(indent) = indent {
        return it.map(|line| {
            if line.len() >= indent {
                line.slice_from(indent..)
            } else {
                line
            }
        }).collect();
    }

    return it.collect();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_code_block() {
        let result = strip_code_block("\n   hello\n  world\n\n\n again\n\n\n".into());
        let expected: Vec<Cow<'static, str>> = vec![
            "  hello".into(),
            " world".into(),
            "".into(),
            "".into(),
            "again".into(),
        ];

        assert_eq!(expected, result);
    }
}
