use backend::models::{Type, Modifier, Package};
use token;

/// Position relative in file where the declaration is present.
pub type Pos = (usize, usize);
pub type Token<T> = token::Token<T, Pos>;

#[derive(Debug, PartialEq, Clone)]
pub struct FieldInit {
    pub name: Token<String>,
    pub value: Token<Value>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Instance {
    pub ty: Type,
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

#[derive(Debug)]
pub struct Field {
    pub modifier: Modifier,
    pub name: String,
    pub ty: Type,
    pub field_as: Option<Token<Value>>,
}

impl Field {
    pub fn is_optional(&self) -> bool {
        match self.modifier {
            Modifier::Optional => true,
            _ => false,
        }
    }
}

#[derive(Debug)]
pub enum Member {
    Field(Field),
    Code(String, Vec<String>),
    Option(Token<OptionDecl>),
    Match(MatchDecl),
}

#[derive(Debug)]
pub struct MatchVariable {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug)]
pub enum MatchCondition {
    /// Match a specific value.
    Value(Token<Value>),
    /// Match a type, and add a binding for the given name that can be resolved in the action.
    Type(MatchVariable),
}

#[derive(Debug)]
pub struct MatchMember {
    pub condition: Token<MatchCondition>,
    pub value: Token<Value>,
}

#[derive(Debug)]
pub struct MatchDecl {
    pub members: Vec<Token<MatchMember>>,
}

pub trait Body {
    fn name(&self) -> &str;
}

impl Body for TupleBody {
    fn name(&self) -> &str {
        &self.name
    }
}

impl Body for TypeBody {
    fn name(&self) -> &str {
        &self.name
    }
}

impl Body for EnumBody {
    fn name(&self) -> &str {
        &self.name
    }
}

impl Body for InterfaceBody {
    fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug)]
pub struct TupleBody {
    pub name: String,
    pub members: Vec<Token<Member>>,
}

#[derive(Debug)]
pub struct InterfaceBody {
    pub name: String,
    pub members: Vec<Token<Member>>,
    pub sub_types: Vec<Token<SubType>>,
}

#[derive(Debug)]
pub struct TypeBody {
    pub name: String,
    pub members: Vec<Token<Member>>,
}

/// Sub-types in interface declarations.
#[derive(Debug)]
pub struct SubType {
    pub name: String,
    pub members: Vec<Token<Member>>,
}

#[derive(Debug)]
pub struct EnumBody {
    pub name: String,
    pub values: Vec<Token<EnumValue>>,
    pub members: Vec<Token<Member>>,
}

#[derive(Debug)]
pub struct EnumValue {
    pub name: String,
    pub arguments: Vec<Token<Value>>,
    pub ordinal: Option<Token<Value>>,
}

#[derive(Debug)]
pub enum Decl {
    Type(TypeBody),
    Tuple(TupleBody),
    Interface(InterfaceBody),
    Enum(EnumBody),
}

impl Decl {
    pub fn name(&self) -> String {
        match *self {
            Decl::Interface(ref interface) => interface.name.clone(),
            Decl::Type(ref ty) => ty.name.clone(),
            Decl::Tuple(ref ty) => ty.name.clone(),
            Decl::Enum(ref ty) => ty.name.clone(),
        }
    }

    pub fn display(&self) -> String {
        match *self {
            Decl::Interface(ref body) => format!("interface {}", body.name),
            Decl::Type(ref body) => format!("type {}", body.name),
            Decl::Tuple(ref body) => format!("tuple {}", body.name),
            Decl::Enum(ref body) => format!("enum {}", body.name),
        }
    }
}

#[derive(Debug)]
pub struct UseDecl {
    pub package: Token<Package>,
    pub alias: Option<String>,
}

#[derive(Debug)]
pub struct File {
    pub package: Token<Package>,
    pub uses: Vec<Token<UseDecl>>,
    pub decls: Vec<Token<Decl>>,
}
