use codeviz::errors as codeviz;
use core::*;
use parser::errors as parser_errors;
use serde_json;

error_chain! {
    links {
        Parser(parser_errors::Error, parser_errors::ErrorKind);
        Codeviz(codeviz::Error, codeviz::ErrorKind);
    }

    foreign_links {
        IO(::std::io::Error);
        Fmt(::std::fmt::Error);
        Serde(serde_json::Error);
    }

    errors {
        Pos(message: String, pos: RpPos) {
            description("position error")
            display("{}", message)
        }

        FieldConflict(message: String, source: RpPos, target: RpPos) {
            description("field conflict")
            display("{}", message)
        }

        DeclMerge(message: String, source: RpPos, target: RpPos) {
            description("declaration merge")
            display("{}", message)
        }

        ExtendEnum(message: String, source: RpPos, enum_pos: RpPos) {
            description("extend enum")
            display("{}", message)
        }

        ReservedField(field_pos: RpPos, reserved_pos: RpPos) {
            description("field reserved")
            display("field reserved")
        }

        MatchConflict(source: RpPos, target: RpPos) {
            description("match conflict")
        }

        /// An instance creation is missing a set of required fields.
        MissingRequired(names: Vec<String>, pos: RpPos, fields: Vec<RpPos>) {
            description("missing required")
        }

        EnumVariantConflict(pos: RpPos, other: RpPos) {
            description("enum value conflict")
        }

        RegisteredTypeConflict(type_id: RpTypeId) {
            description("registered type conflict")
            display("registered type conflict with: {:?}", type_id)
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

impl Error {
    pub fn pos(message: String, pos: RpPos) -> Error {
        ErrorKind::Pos(message, pos).into()
    }

    pub fn field_conflict(field: String, source: RpPos, target: RpPos) -> Error {
        ErrorKind::FieldConflict(field, source, target).into()
    }

    pub fn decl_merge(message: String, source: RpPos, target: RpPos) -> Error {
        ErrorKind::DeclMerge(message, source, target).into()
    }

    pub fn extend_enum(message: String, source: RpPos, enum_pos: RpPos) -> Error {
        ErrorKind::ExtendEnum(message, source, enum_pos).into()
    }

    pub fn reserved_field(field_pos: RpPos, reserved_pos: RpPos) -> Error {
        ErrorKind::ReservedField(field_pos, reserved_pos).into()
    }
}
