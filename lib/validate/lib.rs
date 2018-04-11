extern crate linked_hash_map;
extern crate reproto_core as core;
extern crate serde;
extern crate serde_json;
extern crate serde_yaml;

use linked_hash_map::LinkedHashMap;

/// An numeric value to validate.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Number {
    U32(u32),
    U64(u64),
    I32(i32),
    I64(i64),
    Float(f32),
    Double(f64),
}

/// An opaque value to validate.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Value {
    Number(Number),
    Boolean(bool),
    String(String),
    Any(Box<Value>),
    Object(LinkedHashMap<String, Value>),
    Array(Vec<Value>),
}

/// Format to validate.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Format {
    JSON,
    YAML,
}

/// Validate the given object against a declaration.
pub fn validate(d: &RpDecl, o: &Object) -> Result<()> {
    Ok(())
}
