use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use super::errors::*;
use token;

pub type Pos = (PathBuf, usize, usize);
pub type Token<T> = token::Token<T, Pos>;

#[derive(Debug, PartialEq, Clone)]
pub struct FieldInit {
    pub name: Token<String>,
    pub value: Token<Value>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Instance {
    pub ty: Custom,
    pub arguments: Vec<Token<FieldInit>>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Constant {
    pub prefix: Option<String>,
    pub parts: Vec<String>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Value {
    String(String),
    Number(f64),
    Boolean(bool),
    Identifier(String),
    Type(Type),
    Instance(Token<Instance>),
    Constant(Token<Constant>),
}

#[derive(Debug)]
pub struct OptionDecl {
    pub name: String,
    pub values: Vec<Token<Value>>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Custom {
    pub prefix: Option<String>,
    pub parts: Vec<String>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Type {
    Double,
    Float,
    Signed(Option<usize>),
    Unsigned(Option<usize>),
    Boolean,
    String,
    Bytes,
    Any,
    Custom(Custom),
    Array(Box<Type>),
    Map(Box<Type>, Box<Type>),
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, PartialOrd, Ord)]
pub struct Package {
    pub parts: Vec<String>,
}

impl Package {
    pub fn new(parts: Vec<String>) -> Package {
        Package { parts: parts }
    }

    pub fn join(&self, other: &Package) -> Package {
        let mut parts = self.parts.clone();
        parts.extend(other.parts.clone());
        Package::new(parts)
    }
}

impl ::std::fmt::Display for Package {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "{}", self.parts.join("."))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Modifier {
    Required,
    Optional,
    Repeated,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Modifiers {
    modifiers: HashSet<Modifier>,
}

impl Modifiers {
    pub fn new(modifiers: HashSet<Modifier>) -> Modifiers {
        Modifiers { modifiers: modifiers }
    }

    pub fn test(&self, modifier: &Modifier) -> bool {
        self.modifiers.contains(modifier)
    }
}

#[derive(Debug, Clone)]
pub struct Field {
    pub modifier: Modifier,
    pub name: String,
    pub ty: Type,
    pub field_as: Option<Token<String>>,
}

impl Field {
    pub fn is_optional(&self) -> bool {
        match self.modifier {
            Modifier::Optional => true,
            _ => false,
        }
    }

    pub fn name(&self) -> &str {
        if let Some(ref field) = self.field_as {
            &field.inner
        } else {
            &self.name
        }
    }

    pub fn display(&self) -> String {
        self.name.to_owned()
    }
}

#[derive(Debug, Clone)]
pub struct Code {
    pub context: String,
    pub lines: Vec<String>,
}

pub trait BodyLike {
    fn fields(&self) -> &Vec<Token<Field>>;
    fn codes(&self) -> &Vec<Token<Code>>;
}

impl BodyLike for InterfaceBody {
    fn fields(&self) -> &Vec<Token<Field>> {
        &self.fields
    }

    fn codes(&self) -> &Vec<Token<Code>> {
        &self.codes
    }
}

impl BodyLike for TypeBody {
    fn fields(&self) -> &Vec<Token<Field>> {
        &self.fields
    }

    fn codes(&self) -> &Vec<Token<Code>> {
        &self.codes
    }
}

impl BodyLike for EnumBody {
    fn fields(&self) -> &Vec<Token<Field>> {
        &self.fields
    }

    fn codes(&self) -> &Vec<Token<Code>> {
        &self.codes
    }
}

impl BodyLike for TupleBody {
    fn fields(&self) -> &Vec<Token<Field>> {
        &self.fields
    }

    fn codes(&self) -> &Vec<Token<Code>> {
        &self.codes
    }
}

impl BodyLike for SubType {
    fn fields(&self) -> &Vec<Token<Field>> {
        &self.fields
    }

    fn codes(&self) -> &Vec<Token<Code>> {
        &self.codes
    }
}

#[derive(Debug, Clone)]
pub struct SubType {
    pub name: String,
    pub fields: Vec<Token<Field>>,
    pub codes: Vec<Token<Code>>,
    pub names: Vec<Token<String>>,
}

impl SubType {
    pub fn name(&self) -> String {
        self.names
            .iter()
            .map(|t| t.inner.to_owned())
            .nth(0)
            .unwrap_or_else(|| self.name.clone())
    }
}

#[derive(Debug, Clone)]
pub struct InterfaceBody {
    pub name: String,
    pub fields: Vec<Token<Field>>,
    pub codes: Vec<Token<Code>>,
    pub match_decl: MatchDecl,
    pub sub_types: BTreeMap<String, Token<SubType>>,
}

#[derive(Debug, Clone)]
pub struct TypeBody {
    pub name: String,
    pub fields: Vec<Token<Field>>,
    pub codes: Vec<Token<Code>>,
    pub match_decl: MatchDecl,
    // Set of fields which are reserved for this type.
    pub reserved: HashSet<Token<String>>,
}

impl TypeBody {
    pub fn verify(&self) -> Result<()> {
        for reserved in &self.reserved {
            if let Some(field) = self.fields.iter().find(|f| f.name == reserved.inner) {
                return Err(Error::reserved_field(field.pos.clone(), reserved.pos.clone()));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct TupleBody {
    pub name: String,
    pub fields: Vec<Token<Field>>,
    pub codes: Vec<Token<Code>>,
    pub match_decl: MatchDecl,
}

#[derive(Debug, Clone)]
pub struct EnumValue {
    pub name: String,
    pub arguments: Vec<Token<Value>>,
    pub ordinal: u32,
}

#[derive(Debug, Clone)]
pub struct EnumBody {
    pub name: String,
    pub values: Vec<Token<EnumValue>>,
    pub fields: Vec<Token<Field>>,
    pub codes: Vec<Token<Code>>,
    pub match_decl: MatchDecl,
    pub serialized_as: Option<Token<String>>,
    pub serialized_as_name: bool,
}

#[derive(Clone)]
pub enum Decl {
    Type(TypeBody),
    Interface(InterfaceBody),
    Enum(EnumBody),
    Tuple(TupleBody),
}

impl Decl {
    pub fn name(&self) -> &str {
        match *self {
            Decl::Type(ref body) => &body.name,
            Decl::Interface(ref body) => &body.name,
            Decl::Enum(ref body) => &body.name,
            Decl::Tuple(ref body) => &body.name,
        }
    }

    pub fn display(&self) -> String {
        match *self {
            Decl::Type(ref body) => format!("type {}", body.name),
            Decl::Interface(ref body) => format!("interface {}", body.name),
            Decl::Enum(ref body) => format!("enum {}", body.name),
            Decl::Tuple(ref body) => format!("tuple {}", body.name),
        }
    }
}

/// Simplified types that _can_ be uniquely matched over.
#[derive(Debug, PartialEq, Clone)]
pub enum MatchKind {
    Any,
    Object,
    Array,
    String,
    Boolean,
    Number,
}

#[derive(Debug, Clone)]
pub enum MatchCondition {
    /// Match a specific value.
    Value(Token<Value>),
    /// Match a type, and add a binding for the given name that can be resolved in the action.
    Type(MatchVariable),
}

#[derive(Debug, Clone)]
pub struct MatchMember {
    pub condition: Token<MatchCondition>,
    pub value: Token<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchVariable {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone)]
pub struct MatchDecl {
    pub by_value: Vec<(Token<Value>, Token<MatchMember>)>,
    pub by_type: Vec<(MatchKind, Token<MatchMember>)>,
}

impl MatchDecl {
    pub fn new() -> MatchDecl {
        MatchDecl {
            by_value: Vec::new(),
            by_type: Vec::new(),
        }
    }

    pub fn identify_match_kind(&self, variable: &MatchVariable) -> MatchKind {
        match variable.ty {
            Type::Double |
            Type::Float |
            Type::Signed(_) |
            Type::Unsigned(_) => MatchKind::Number,
            Type::Boolean => MatchKind::Boolean,
            Type::String | Type::Bytes => MatchKind::String,
            Type::Any => MatchKind::Any,
            Type::Custom(_) | Type::Map(_, _) => MatchKind::Object,
            Type::Array(_) => MatchKind::Array,
        }
    }

    pub fn push(&mut self, member: Token<MatchMember>) -> Result<()> {
        match member.condition.inner {
            MatchCondition::Type(ref variable) => {
                let match_kind = self.identify_match_kind(variable);

                {
                    // conflicting when type matches
                    let result =
                        self.by_type.iter().find(|e| e.0 == match_kind || e.0 == MatchKind::Any);

                    if let Some(&(_, ref existing_value)) = result {
                        let err = ErrorKind::MatchConflict(member.condition.pos.clone(),
                                                           existing_value.condition.pos.clone());
                        return Err(err.into());
                    }
                }

                self.by_type.push((match_kind, member.clone()));
            }
            MatchCondition::Value(ref value) => {
                {
                    // conflicting when value matches
                    let result = self.by_value.iter().find(|e| e.0.inner == value.inner);

                    if let Some(&(_, ref existing_value)) = result {
                        let err = ErrorKind::MatchConflict(member.condition.pos.clone(),
                                                           existing_value.condition.pos.clone());
                        return Err(err.into());
                    }
                }

                self.by_value.push((value.clone(), member.clone()));
            }
        }

        Ok(())
    }
}
