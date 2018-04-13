extern crate reproto_core as core;
extern crate reproto_lexer as lexer;

use core::errors::Result;
use core::{Loc, RpNumber, RpPackage};
use lexer::Token;
use std::borrow::Cow;
use std::ops;
use std::vec;

/// A value that can be error-recovered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErrorRecovery<'input, T> {
    Error(Vec<(usize, Token<'input>, usize)>),
    Value(T),
}

impl<'input, T> ErrorRecovery<'input, T> {
    /// Return the value or an error.
    pub fn recover(self) -> Result<T> {
        use self::ErrorRecovery::*;

        match self {
            Error(_) => Err("value not available".into()),
            Value(value) => Ok(value),
        }
    }
}

impl<'input, T> From<T> for ErrorRecovery<'input, T> {
    fn from(value: T) -> ErrorRecovery<'input, T> {
        ErrorRecovery::Value(value)
    }
}

/// Items can be commented and have attributes.
///
/// This is an intermediate structure used to return these properties.
///
/// ```ignore
/// /// This is a comment.
/// #[foo]
/// #[foo(value = "hello")]
/// <item>
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct Item<'input, T> {
    pub comment: Vec<Cow<'input, str>>,
    pub attributes: Vec<Loc<Attribute<'input>>>,
    pub item: Loc<T>,
}

/// Item derefs into target.
impl<'input, T> ops::Deref for Item<'input, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        Loc::borrow(&self.item)
    }
}

/// Name value pair.
///
/// Is associated with attributes:
///
/// ```ignore
/// #[attribute(name = <value>)]
/// ```
#[derive(Debug, PartialEq, Eq)]
pub enum AttributeItem<'input> {
    Word(Loc<Value<'input>>),
    NameValue {
        name: Loc<Cow<'input, str>>,
        value: Loc<Value<'input>>,
    },
}

/// An attribute.
///
/// Attributes are metadata associated with elements.
///
/// ```ignore
/// #[word]
/// ```
///
/// or:
///
/// ```ignore
/// #[name_value(foo = <value>, bar = <value>)]
/// ```
#[derive(Debug, PartialEq, Eq)]
pub enum Attribute<'input> {
    Word(Loc<Cow<'input, str>>),
    List(Loc<Cow<'input, str>>, Vec<AttributeItem<'input>>),
}

/// A type.
///
/// For example: `u32`, `::Relative::Name`, or `bytes`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type<'input> {
    Double,
    Float,
    Signed {
        size: usize,
    },
    Unsigned {
        size: usize,
    },
    Boolean,
    String,
    Bytes,
    Any,
    /// ISO-8601 for date and time.
    DateTime,
    Name {
        name: Name<'input>,
    },
    Array {
        inner: Box<Type<'input>>,
    },
    Map {
        key: Box<Type<'input>>,
        value: Box<Type<'input>>,
    },
}

/// Any kind of declaration.
#[derive(Debug, PartialEq, Eq)]
pub enum Decl<'input> {
    Type(Item<'input, TypeBody<'input>>),
    Tuple(Item<'input, TupleBody<'input>>),
    Interface(Item<'input, InterfaceBody<'input>>),
    Enum(Item<'input, EnumBody<'input>>),
    Service(Item<'input, ServiceBody<'input>>),
}

impl<'input> Decl<'input> {
    /// Get the local name for the declaration.
    pub fn name(&self) -> &str {
        use self::Decl::*;

        match *self {
            Type(ref body) => &body.name,
            Tuple(ref body) => &body.name,
            Interface(ref body) => &body.name,
            Enum(ref body) => &body.name,
            Service(ref body) => &body.name,
        }
    }

    /// Get all the sub-declarations of this declaraiton.
    pub fn decls(&self) -> Decls {
        use self::Decl::*;

        let decls = match *self {
            Type(ref body) => body.decls(),
            Tuple(ref body) => body.decls(),
            Interface(ref body) => body.decls(),
            Enum(ref body) => body.decls(),
            Service(ref body) => body.decls(),
        };

        Decls {
            iter: decls.into_iter(),
        }
    }
}

pub struct Decls<'a, 'input: 'a> {
    iter: vec::IntoIter<&'a Decl<'input>>,
}

impl<'a, 'input: 'a> Iterator for Decls<'a, 'input> {
    type Item = &'a Decl<'input>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

/// The body of an enum declaration.
///
/// ```ignore
/// enum <name> as <ty> {
///   <variants>
///
///   <members>
/// }
/// ```
///
/// Note: members must only be options.
#[derive(Debug, PartialEq, Eq)]
pub struct EnumBody<'input> {
    pub name: Cow<'input, str>,
    pub ty: Loc<Type<'input>>,
    pub variants: Vec<Item<'input, EnumVariant<'input>>>,
    pub members: Vec<EnumMember<'input>>,
}

impl<'input> EnumBody<'input> {
    /// Access all inner declarations.
    fn decls(&self) -> Vec<&Decl<'input>> {
        Vec::new()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct EnumVariant<'input> {
    pub name: Loc<Cow<'input, str>>,
    pub argument: Option<Loc<Value<'input>>>,
}

/// A member in a tuple, type, or interface.
#[derive(Debug, PartialEq, Eq)]
pub enum EnumMember<'input> {
    Code(Loc<Code<'input>>),
}

/// A field.
///
/// ```ignore
/// <name><modifier>: <ty> as <field_as>
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct Field<'input> {
    pub required: bool,
    pub name: Cow<'input, str>,
    pub ty: Loc<ErrorRecovery<'input, Type<'input>>>,
    pub field_as: Option<String>,
}

/// A file.
///
/// ```ignore
/// <uses>
///
/// <options>
///
/// <decls>
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct File<'input> {
    pub comment: Vec<Cow<'input, str>>,
    pub attributes: Vec<Loc<Attribute<'input>>>,
    pub uses: Vec<Loc<UseDecl<'input>>>,
    pub decls: Vec<Decl<'input>>,
}

impl<'input> Field<'input> {
    pub fn is_optional(&self) -> bool {
        !self.required
    }
}

/// A name.
///
/// Either:
///
/// ```ignore
/// ::Relative::Name
/// ```
///
/// Or:
///
/// ```ignore
/// <prefix::>Absolute::Name
/// ```
///
/// Note: prefixes names are _always_ imported with `UseDecl`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Name<'input> {
    Relative {
        parts: Vec<Loc<Cow<'input, str>>>,
    },
    Absolute {
        prefix: Option<Loc<Cow<'input, str>>>,
        parts: Vec<Loc<Cow<'input, str>>>,
    },
}

/// The body of an interface declaration
///
/// ```ignore
/// interface <name> {
///   <members>
///   <sub_types>
/// }
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct InterfaceBody<'input> {
    pub name: Cow<'input, str>,
    pub members: Vec<TypeMember<'input>>,
    pub sub_types: Vec<Item<'input, SubType<'input>>>,
}

impl<'input> InterfaceBody<'input> {
    /// Access all inner declarations.
    fn decls(&self) -> Vec<&Decl<'input>> {
        let mut out = Vec::new();

        for m in &self.members {
            if let TypeMember::InnerDecl(ref decl) = *m {
                out.push(decl);
            }
        }

        out
    }

    /// Access all fields.
    pub fn fields(&self) -> Vec<&Field<'input>> {
        let mut out = Vec::new();

        for m in &self.members {
            if let TypeMember::Field(ref field) = *m {
                out.push(Loc::borrow(&field.item));
            }
        }

        out
    }
}

/// A contextual code-block.
#[derive(Debug, PartialEq, Eq)]
pub struct Code<'input> {
    pub attributes: Vec<Loc<Attribute<'input>>>,
    pub context: Loc<Cow<'input, str>>,
    pub content: Vec<Cow<'input, str>>,
}

/// A member in a tuple, type, or interface.
#[derive(Debug, PartialEq, Eq)]
pub enum TypeMember<'input> {
    Field(Item<'input, Field<'input>>),
    Code(Loc<Code<'input>>),
    InnerDecl(Decl<'input>),
}

/// The body of a service declaration.
///
/// ```ignore
/// service <name> {
///   <members>
/// }
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct ServiceBody<'input> {
    pub name: Cow<'input, str>,
    pub members: Vec<ServiceMember<'input>>,
}

impl<'input> ServiceBody<'input> {
    /// Access all inner declarations.
    fn decls(&self) -> Vec<&Decl<'input>> {
        let mut out = Vec::new();

        for m in &self.members {
            if let ServiceMember::InnerDecl(ref decl) = *m {
                out.push(decl);
            }
        }

        out
    }
}

/// A member of a service declaration.
#[derive(Debug, PartialEq, Eq)]
pub enum ServiceMember<'input> {
    Endpoint(Item<'input, Endpoint<'input>>),
    InnerDecl(Decl<'input>),
}

/// The argument in and endpoint.
#[derive(Debug, PartialEq, Eq)]
pub struct EndpointArgument<'input> {
    pub ident: Loc<Cow<'input, str>>,
    pub channel: Loc<Channel<'input>>,
}

/// An endpoint
///
/// ```ignore
/// <id>(<arguments>) -> <response> as <alias> {
///   <options>
/// }
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct Endpoint<'input> {
    pub id: Loc<Cow<'input, str>>,
    pub alias: Option<String>,
    pub arguments: Vec<EndpointArgument<'input>>,
    pub response: Option<Loc<Channel<'input>>>,
}

/// Describes how data is transferred over a channel.
///
/// ```ignore
/// Unary(stream <ty>)
/// Streaming(<ty>)
/// ```
#[derive(Debug, PartialEq, Eq)]
pub enum Channel<'input> {
    /// Single send.
    Unary { ty: Type<'input> },
    /// Multiple sends.
    Streaming { ty: Type<'input> },
}

/// The body of a sub-type
///
/// ```ignore
/// <name> as <alias> {
///     <members>
/// }
/// ```
/// Sub-types in interface declarations.
#[derive(Debug, PartialEq, Eq)]
pub struct SubType<'input> {
    pub name: Loc<Cow<'input, str>>,
    pub members: Vec<TypeMember<'input>>,
    pub alias: Option<Loc<Value<'input>>>,
}

/// The body of a tuple
///
/// ```ignore
/// tuple <name> {
///     <members>
/// }
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct TupleBody<'input> {
    pub name: Cow<'input, str>,
    pub members: Vec<TypeMember<'input>>,
}

impl<'input> TupleBody<'input> {
    /// Access all inner declarations.
    fn decls(&self) -> Vec<&Decl<'input>> {
        let mut out = Vec::new();

        for m in &self.members {
            if let TypeMember::InnerDecl(ref decl) = *m {
                out.push(decl);
            }
        }

        out
    }

    /// Access all fields.
    pub fn fields(&self) -> Vec<&Field<'input>> {
        let mut out = Vec::new();

        for m in &self.members {
            if let TypeMember::Field(ref field) = *m {
                out.push(Loc::borrow(&field.item));
            }
        }

        out
    }
}

/// The body of a type
///
/// ```ignore
/// type <name> {
///     <members>
/// }
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct TypeBody<'input> {
    pub name: Cow<'input, str>,
    pub members: Vec<TypeMember<'input>>,
}

impl<'input> TypeBody<'input> {
    /// Access all inner declarations.
    fn decls(&self) -> Vec<&Decl<'input>> {
        let mut out = Vec::new();

        for m in &self.members {
            if let TypeMember::InnerDecl(ref decl) = *m {
                out.push(decl);
            }
        }

        out
    }

    /// Access all fields.
    pub fn fields(&self) -> Vec<&Field<'input>> {
        let mut out = Vec::new();

        for m in &self.members {
            if let TypeMember::Field(ref field) = *m {
                out.push(Loc::borrow(&field.item));
            }
        }

        out
    }
}

/// A use declaration
///
/// ```ignore
/// use <package> "<version req> as <alias>
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct UseDecl<'input> {
    pub package: Loc<RpPackage>,
    pub range: Option<Loc<String>>,
    pub alias: Option<Loc<Cow<'input, str>>>,
}

/// A literal value
///
/// For example, `"string"`, `42.0`, and `foo`.
#[derive(Debug, PartialEq, Eq)]
pub enum Value<'input> {
    String(String),
    Number(RpNumber),
    Identifier(Cow<'input, str>),
    Array(Vec<Loc<Value<'input>>>),
}

/// A part of a step.
#[derive(Debug, PartialEq, Eq)]
pub enum PathPart<'input> {
    Variable(Cow<'input, str>),
    Segment(String),
}

/// A step in a path specification.
#[derive(Debug, PartialEq, Eq)]
pub struct PathStep<'input> {
    pub parts: Vec<PathPart<'input>>,
}

/// A path specification.
#[derive(Debug, PartialEq, Eq)]
pub struct PathSpec<'input> {
    pub steps: Vec<PathStep<'input>>,
}
