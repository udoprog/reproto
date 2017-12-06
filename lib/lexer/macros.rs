/// Take until the givern pattern matches.
/// This will return content up-until the pattern matches, and consume the pattern itself.
#[macro_export]
macro_rules! take_until {
    ($slf:expr, $start:expr, $first:pat $(| $rest:pat)*) => {{
        let mut __end = $start;
        let mut __content_end = $start;

        loop {
            if let Some((e, c)) = $slf.one() {
                __content_end = e;

                match c {
                    $first $(| $rest)* => {
                        let (e, _) = take!($slf, e, $first $(| $rest)*);
                        __end = e;
                        break;
                    }
                    _ => {
                        __end = e;
                        $slf.step();
                    }
                }
            } else {
                __content_end = $slf.source_len;
                break;
            }
        }

        (__end, &$slf.source_str[$start..__content_end])
    }}
}

/// Take while pattern matches.
#[macro_export]
macro_rules! take {
    ($slf:expr, $start:expr, $first:pat $(| $rest:pat)*) => {{
        let mut __end: usize = $start;

        loop {
            if let Some((__pos, __c)) = $slf.one() {
                __end = __pos;

                match __c {
                    $first $(| $rest)* => {},
                    _ => break,
                }

                $slf.step();
            } else {
                __end = $slf.source_len;
                break;
            }
        }

        (__end, &$slf.source_str[$start..__end])
    }}
}
