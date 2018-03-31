use genco::{IntoTokens, Rust, Tokens};

/// Documentation comments.
pub struct Comments<'el, S: 'el>(pub &'el [S]);

impl<'el, S: 'el + AsRef<str>> IntoTokens<'el, Rust<'el>> for Comments<'el, S> {
    fn into_tokens(self) -> Tokens<'el, Rust<'el>> {
        let mut t = Tokens::new();

        for c in self.0.iter() {
            t.push(toks!["/// ", c.as_ref()]);
        }

        t
    }
}
