use Opaque;
use core;
use core::errors::Result;
use format;
use linked_hash_map::LinkedHashMap;
use serde_json as json;
use sir::{FieldSir, Sir};
use utils::is_datetime;

#[derive(Debug)]
pub struct Json;

impl format::Format for Json {
    fn decode(&self, object: &core::Source) -> Result<Sir> {
        let mut der = json::Deserializer::from_reader(object.read()?).into_iter::<json::Value>();

        let value: Result<json::Value> = der.next()
            .ok_or_else(|| format!("Expected at least one JSON value").into())
            .and_then(|v| v.map_err(|e| format!("Bad JSON: {}", e).into()));

        let value = value?;
        let sir = from_json(&value)?;

        Ok(sir)
    }
}

impl format::Object for json::Map<String, json::Value> {
    type Value = json::Value;

    fn get(&self, key: &str) -> Option<&Self::Value> {
        self.get(key)
    }
}

impl format::Value for json::Value {
    fn as_object(&self) -> Option<&format::Object<Value = Self>> {
        match *self {
            json::Value::Object(ref object) => Some(object as &format::Object<Value = Self>),
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match *self {
            json::Value::String(ref string) => Some(string),
            _ => None,
        }
    }
}

/// Calculate fingerprint from JSON value.
fn from_json(value: &json::Value) -> Result<Sir> {
    let f = match *value {
        json::Value::Number(ref number) if number.is_u64() => {
            let number = number
                .as_u64()
                .ok_or_else(|| format!("Expected u64, got: {}", number))?;

            Sir::U64(Opaque::new(vec![number]))
        }
        json::Value::Number(ref number) if number.is_i64() => {
            let number = number
                .as_i64()
                .ok_or_else(|| format!("Expected i64, got: {}", number))?;

            Sir::I64(Opaque::new(vec![number]))
        }
        json::Value::Number(ref number) => {
            // Find best representation, float or double.

            let number = number
                .as_f64()
                .ok_or_else(|| format!("Expected f64, got: {}", number))?;

            let diff = (((number as f32) as f64) - number).abs();

            if diff != 0f64 {
                Sir::Double
            } else {
                Sir::Float
            }
        }
        json::Value::Bool(_) => Sir::Boolean,
        json::Value::String(ref string) => {
            if is_datetime(string) {
                Sir::DateTime(Opaque::new(vec![string.to_string()]))
            } else {
                Sir::String(Opaque::new(vec![string.to_string()]))
            }
        }
        json::Value::Null => Sir::Any,
        json::Value::Array(ref array) => Sir::process_array(&array, from_json)?,
        json::Value::Object(ref map) => {
            let mut entries = LinkedHashMap::new();

            for (key, value) in map {
                let value = from_json(value)?;

                let field = FieldSir {
                    optional: value == Sir::Any,
                    field: value,
                };

                entries.insert(key.to_string(), field);
            }

            Sir::Object(entries)
        }
    };

    return Ok(f);
}
