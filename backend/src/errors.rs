use codeviz_common::errors as codeviz;
use core::{ErrorPos, RpName, WithPos, errors as core};
use parser::errors as parser;
use repository::errors as repository;
use serde_json as json;

error_chain! {
    links {
        Parser(parser::Error, parser::ErrorKind);
        Core(core::Error, core::ErrorKind);
        Codeviz(codeviz::Error, codeviz::ErrorKind);
        Repository(repository::Error, repository::ErrorKind);
    }

    foreign_links {
        Io(::std::io::Error);
        Fmt(::std::fmt::Error);
        Json(json::Error);
        BorrowMutError(::std::cell::BorrowMutError);
        BorrowError(::std::cell::BorrowError);
    }

    errors {
        Pos(message: String, pos: ErrorPos) {
            description("position error")
            display("{}", message)
        }

        /// An instance creation is missing a set of required fields.
        MissingRequired(names: Vec<String>, pos: ErrorPos, fields: Vec<ErrorPos>) {
            description("missing required")
        }

        Overflow(pos: ErrorPos) {
        }

        EnumVariantConflict(pos: ErrorPos, other: ErrorPos) {
            description("enum value conflict")
        }

        FieldConflict(message: String, source: ErrorPos, target: ErrorPos) {
            description("field conflict")
            display("{}", message)
        }

        Errors(errors: Vec<Error>) {
            description("errors")
            display("encountered {} error(s)", errors.len())
        }

        MissingBackend {
        }

        RegisteredTypeConflict(name: RpName) {
            description("registered type conflict")
            display("registered type conflict with: {}", name)
        }

        MissingPrefix(prefix: String) {
            description("missing prefix")
            display("missing prefix: {}", prefix)
        }
    }
}

impl Error {
    pub fn pos(message: String, pos: ErrorPos) -> Error {
        ErrorKind::Pos(message, pos).into()
    }
}

impl WithPos for Error {
    fn with_pos<E: Into<ErrorPos>>(self, pos: E) -> Self {
        use self::ErrorKind::*;

        match self.kind() {
            &Pos(..) => self,
            &MissingRequired(..) => self,
            &Overflow(..) => self,
            &EnumVariantConflict(..) => self,
            &FieldConflict(..) => self,
            &Errors(..) => self,
            _ => {
                let message = format!("{}", self);
                self.chain_err(|| ErrorKind::Pos(message, pos.into()))
            }
        }
    }
}
