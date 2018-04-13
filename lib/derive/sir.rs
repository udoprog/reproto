use Opaque;
use core::errors::Result;
use format;
use linked_hash_map::LinkedHashMap;
use std::collections::HashSet;
use std::mem;

/// Results from calling `Sir::test_interface`.
struct InterfaceTestResult {
    pub tag: String,
    pub sub_types: Vec<SubTypeSir>,
}

/// Structural, intermediate representation.
///
/// This describes the structure of a document and permits transformations.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Sir {
    U64(Opaque<Vec<u64>>),
    I64(Opaque<Vec<i64>>),
    Float,
    Double,
    Boolean,
    String(Opaque<Vec<String>>),
    DateTime(Opaque<Vec<String>>),
    Any,
    Object(LinkedHashMap<String, FieldSir>),
    Interface(String, Vec<SubTypeSir>),
    Array(Box<Sir>),
    Tuple(Vec<FieldSir>),
}

impl Sir {
    /// Recursively simplify this SIR into one that is only used for hashing.
    fn hash(&self) -> HashSir {
        match *self {
            Sir::U64(_)
            | Sir::I64(_)
            | Sir::Float
            | Sir::Double
            | Sir::Boolean
            | Sir::String(_)
            | Sir::DateTime(_)
            | Sir::Any => HashSir::Scalar,
            Sir::Object(ref fields) => {
                HashSir::Object(fields.iter().map(|(k, v)| (k.clone(), v.hash())).collect())
            }
            Sir::Interface(ref key, ref sub_types) => HashSir::Interface(
                key.to_string(),
                sub_types.iter().map(SubTypeSir::hash).collect(),
            ),
            Sir::Array(ref inner) => HashSir::Array(Box::new(inner.hash())),
            Sir::Tuple(ref inner) => HashSir::Tuple(inner.iter().map(FieldSir::hash).collect()),
        }
    }

    /// Check if this is an object.
    fn is_object(&self) -> bool {
        use self::Sir::*;

        match *self {
            Object(_) => true,
            _ => false,
        }
    }

    /// Check if this is a string.
    fn is_string(&self) -> bool {
        use self::Sir::*;

        match *self {
            String(_) => true,
            _ => false,
        }
    }

    /// Check if this is an object.
    fn as_object(&self) -> Option<&LinkedHashMap<String, FieldSir>> {
        use self::Sir::*;

        match *self {
            Object(ref object) => Some(object),
            _ => None,
        }
    }

    /// Refine this SIR with another.
    fn refine(&mut self, other: &Sir) -> Result<()> {
        // test for replacements
        let replace = match (&*self, other) {
            (&Sir::U64(ref examples), &Sir::I64(ref other)) => {
                let mut examples: Vec<_> = examples.iter().map(|v| *v as i64).collect();
                examples.extend(other.iter().cloned());
                Some(Sir::I64(Opaque::new(examples)))
            }
            (&Sir::Float, &Sir::Double) => Some(Sir::Double),
            (&Sir::U64(_), &Sir::Float) | (&Sir::I64(_), &Sir::Float) => Some(Sir::Float),
            (&Sir::U64(_), &Sir::Double) | (&Sir::I64(_), &Sir::Double) => Some(Sir::Double),
            // current value unknown, replace with other.
            (&Sir::Any, other) if *other != Sir::Any => Some(other.clone()),
            _ => None,
        };

        if let Some(replace) = replace {
            mem::replace(self, replace);
            return Ok(());
        }

        match (self, other) {
            (&mut Sir::U64(ref mut examples), &Sir::U64(ref other)) => {
                examples.extend(other.iter().cloned());
            }
            (&mut Sir::I64(ref mut examples), &Sir::I64(ref other)) => {
                examples.extend(other.iter().cloned());
            }
            // sign change
            (&mut Sir::Float, &Sir::Float) => {}
            (&mut Sir::Double, &Sir::Double) => {}
            (&mut Sir::Boolean, &Sir::Boolean) => {}
            (&mut Sir::String(ref mut examples), &Sir::String(ref other)) => {
                examples.extend(other.iter().cloned());
            }
            (&mut Sir::DateTime(ref mut examples), &Sir::DateTime(ref other)) => {
                examples.extend(other.iter().cloned());
            }
            (&mut Sir::Object(ref mut entries), &Sir::Object(ref other)) => {
                for ((kl, vl), (kr, vr)) in entries.iter_mut().zip(other.iter()) {
                    if kl != kr {
                        return Err(format!("Expected object key {}, but got {}", kl, kr).into());
                    }

                    vl.refine(vr)?;
                }
            }
            (&mut Sir::Interface(_, ref mut _entries), &Sir::Interface(_, ref _other)) => {}
            (&mut Sir::Array(ref mut inner), &Sir::Array(ref other)) => {
                return inner.refine(other);
            }
            (&mut Sir::Tuple(ref mut inner), &Sir::Tuple(ref other)) => {
                for (inner, other) in inner.iter_mut().zip(other.iter()) {
                    inner.refine(other)?;
                }
            }
            // Nothing to refine :(
            (_, &Sir::Any) => {}
            (current, other) => {
                return Err(format!("{:?} cannot be refined by {:?}", current, other).into());
            }
        }

        Ok(())
    }

    /// Process the given array.
    pub fn process_array<T: format::Value, F>(array: &[T], from_item: F) -> Result<Sir>
    where
        F: Fn(&T) -> Result<Sir>,
    {
        let mut children = Vec::new();
        let mut values = Vec::new();
        let mut unique: Vec<Sir> = Vec::new();

        for item in array {
            let f = from_item(item)?;

            let contained = if let Some(current) = unique.iter_mut().find(|e| e.hash() == f.hash())
            {
                current.refine(&f)?;
                true
            } else {
                false
            };

            if !contained {
                unique.push(f.clone());
            }

            children.push(f);
            values.push(item);
        }

        let mut it = unique.into_iter();

        if let Some(first) = it.next() {
            if it.next().is_some() {
                if let Some(InterfaceTestResult { tag, sub_types }) =
                    Self::test_interface(&children, &values)?
                {
                    return Ok(Sir::Interface(tag, sub_types));
                }

                // All tuple fields are required.
                let children = children
                    .into_iter()
                    .map(|f| FieldSir {
                        optional: false,
                        field: f,
                    })
                    .collect();

                return Ok(Sir::Tuple(children));
            }

            return Ok(Sir::Array(Box::new(first)));
        }

        Ok(Sir::Array(Box::new(Sir::Any)))
    }

    /// Test if the given array is an interface.
    ///
    /// Interface detection happens by eliminating all common fields across a set of JSON
    /// fingerprints, and identifying if the common field can be used as an identifier.
    fn test_interface<T: format::Value>(
        children: &[Sir],
        values: &[&T],
    ) -> Result<Option<InterfaceTestResult>> {
        // check if interface
        if !children.iter().all(Sir::is_object) {
            return Ok(None);
        }

        let children = children.iter().flat_map(Sir::as_object).collect::<Vec<_>>();
        let values = values
            .iter()
            .flat_map(|v| v.as_object())
            .collect::<Vec<_>>();

        let mut common_keys = HashSet::new();
        let mut it = children.iter();

        if let Some(first) = it.next() {
            common_keys.extend(
                first
                    .iter()
                    .filter(|&(_, v)| v.field.is_string())
                    .map(|(k, _)| k.clone()),
            );

            while let Some(other) = it.next() {
                // Only strings can be used as type keys.
                common_keys.retain(|k| other.get(k).map(|v| v.field.is_string()).unwrap_or(false));
            }
        }

        let tag = match pick_tag(&common_keys)? {
            Some(tag) => tag,
            None => return Ok(None),
        };

        let mut out = Vec::new();

        for (child, value) in children.iter().zip(values.iter()) {
            let structure: LinkedHashMap<String, FieldSir> = child
                .iter()
                .filter(|&(key, _)| key != tag)
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();

            let name = value
                .get(tag)
                .ok_or_else(|| format!("Missing common key: {}", tag))
                .map(format::Value::as_str)
                .and_then(|s| s.ok_or_else(|| format!("Expected string as common key: {}", tag)))?
                .to_string();

            out.push(SubTypeSir {
                name: name,
                structure: structure,
            });
        }

        return Ok(Some(InterfaceTestResult {
            tag: tag.to_string(),
            sub_types: out,
        }));

        /// Pick the tag used to determine sub-type.
        ///
        /// Some keys get priority, since they are default values for many frameworks.
        fn pick_tag<'a>(common_keys: &'a HashSet<String>) -> Result<Option<&'a str>> {
            if common_keys.contains("type") {
                return Ok(Some("type"));
            }

            if common_keys.contains("@class") {
                return Ok(Some("@class"));
            }

            if common_keys.contains("kind") {
                return Ok(Some("kind"));
            }

            Ok(None)
        }
    }
}

/// The SIR of a field.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct FieldSir {
    pub optional: bool,
    pub field: Sir,
}

impl FieldSir {
    /// Simplify the given SIR into something useful for looking up similar structures.
    fn hash(&self) -> FieldHashSir {
        FieldHashSir {
            field: self.field.hash(),
        }
    }

    fn refine(&mut self, other: &FieldSir) -> Result<()> {
        // relax optionality
        if !self.optional && other.optional {
            self.optional = true;
        }

        self.field.refine(&other.field)
    }
}

/// Describes an interface sub-type.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct SubTypeSir {
    /// Name of the sub-type.
    pub name: String,
    /// The structure of the sub-type, without the type-key.
    pub structure: LinkedHashMap<String, FieldSir>,
}

impl SubTypeSir {
    /// Simplify the given SIR into something useful for looking up similar structures.
    fn hash(&self) -> SubTypeHashSir {
        SubTypeHashSir {
            name: self.name.clone(),
            structure: self.structure
                .iter()
                .map(|(k, v)| (k.clone(), v.hash()))
                .collect(),
        }
    }
}

// Hash types

/// Simplified SIR used for hashing common lookups.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum HashSir {
    Scalar,
    Object(LinkedHashMap<String, FieldHashSir>),
    Interface(String, Vec<SubTypeHashSir>),
    Array(Box<HashSir>),
    Tuple(Vec<FieldHashSir>),
}

/// The SIR of a field.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct FieldHashSir {
    pub field: HashSir,
}

/// Describes an interface sub-type suitable for looking up similar structures.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct SubTypeHashSir {
    /// Name of the sub-type.
    pub name: String,
    /// The structure of the sub-type, without the type-key.
    pub structure: LinkedHashMap<String, FieldHashSir>,
}
