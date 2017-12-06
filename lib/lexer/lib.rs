//! Lexer used for parsing reproto manifests.

extern crate num;
extern crate reproto_core as core;

#[macro_use]
pub(crate) mod macros;
pub(crate) mod token;
pub(crate) mod lexer;
pub(crate) mod path_token;
pub(crate) mod path_lexer;
pub mod errors;

pub use self::errors::Error;
pub use self::path_lexer::path_lex;
pub use self::lexer::lex;
pub use self::path_token::PathToken;
pub use self::token::Token;
