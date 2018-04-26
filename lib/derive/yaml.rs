use core;
use core::errors::Result;
use format;
use linked_hash_map::LinkedHashMap;
use serde_yaml as yaml;
use sir::{FieldSir, Sir};
use utils::is_datetime;
use Opaque;

#[derive(Debug)]
pub struct Yaml;

impl format::Format for Yaml {
    fn decode(&self, object: &core::Source) -> Result<Sir> {
        let value = yaml::from_reader(object.read()?).map_err(|e| format!("Bad YAML: {}", e))?;
        Ok(from_yaml(&value)?)
    }
}

impl format::Object for yaml::Mapping {
    type Value = yaml::Value;

    fn get(&self, key: &str) -> Option<&Self::Value> {
        self.get(&yaml::Value::String(key.to_string()))
    }
}

impl format::Value for yaml::Value {
    fn as_object(&self) -> Option<&format::Object<Value = Self>> {
        match *self {
            yaml::Value::Mapping(ref mapping) => Some(mapping as &format::Object<Value = Self>),
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match *self {
            yaml::Value::String(ref string) => Some(string),
            _ => None,
        }
    }
}

/// Calculate fingerprint from YAML value.
fn from_yaml(value: &yaml::Value) -> Result<Sir> {
    let f = match *value {
        yaml::Value::Number(ref number) if number.is_u64() => {
            let number = number
                .as_u64()
                .ok_or_else(|| format!("Expected u64, got: {}", number))?;

            Sir::U64(Opaque::new(vec![number]))
        }
        yaml::Value::Number(ref number) if number.is_i64() => {
            let number = number
                .as_i64()
                .ok_or_else(|| format!("Expected i64, got: {}", number))?;

            Sir::I64(Opaque::new(vec![number]))
        }
        yaml::Value::Number(ref number) => {
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
        yaml::Value::Bool(_) => Sir::Boolean,
        yaml::Value::String(ref string) => {
            if is_datetime(string) {
                Sir::DateTime(Opaque::new(vec![string.to_string()]))
            } else {
                Sir::String(Opaque::new(vec![string.to_string()]))
            }
        }
        yaml::Value::Null => Sir::Any,
        yaml::Value::Sequence(ref sequence) => Sir::process_array(&sequence, from_yaml)?,
        yaml::Value::Mapping(ref mapping) => {
            let mut entries = LinkedHashMap::new();

            for (key, value) in mapping {
                let key = key.as_str()
                    .ok_or_else(|| format!("Expected string key: {:?}", key))?;

                let value = from_yaml(value)?;

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
