use super::ErrorPos;
use super::with_pos::WithPos;
use extern_mime;

error_chain! {
    foreign_links {
        IO(::std::io::Error);
        Fmt(::std::fmt::Error);
    }

    errors {
        Pos(message: String, pos: ErrorPos) {
            description("position error")
            display("{}", message)
        }

        DeclMerge(message: String, source: ErrorPos, target: ErrorPos) {
            description("declaration merge")
            display("{}", message)
        }

        FieldConflict(message: String, source: ErrorPos, target: ErrorPos) {
            description("field conflict")
            display("{}", message)
        }

        ExtendEnum(message: String, source: ErrorPos, enum_pos: ErrorPos) {
            description("extend enum")
            display("{}", message)
        }

        ReservedField(field_pos: ErrorPos, reserved_pos: ErrorPos) {
            description("field reserved")
            display("field reserved")
        }

        MatchConflict(source: ErrorPos, target: ErrorPos) {
            description("match conflict")
        }

        MimeFromStrError(error: extern_mime::FromStrError) {
            description("couldn't parse mime type")
            display("{:?}", error)
        }

        InvalidOrdinal {
        }

        /// Error thrown by Rc::get_mut
        RcGetMut {
        }

        RcTryUnwrap {
        }

        Overflow {
        }
    }
}

impl WithPos for Error {
    fn with_pos<E: Into<ErrorPos>>(self, pos: E) -> Self {
        use self::ErrorKind::*;

        match self.kind() {
            &Pos(..) => self,
            &DeclMerge(..) => self,
            &FieldConflict(..) => self,
            &ExtendEnum(..) => self,
            &ReservedField(..) => self,
            &MatchConflict(..) => self,
            _ => {
                let message = format!("{}", &self);
                self.chain_err(|| ErrorKind::Pos(message, pos.into()))
            }
        }
    }
}
