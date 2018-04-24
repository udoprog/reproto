extern crate reproto_core as core;
extern crate reproto_lexer as lexer;

use core::errors::Result;
use core::{Loc, RpNumber, RpPackage};
use lexer::Token;
use std::ops;
use std::vec;

/// A value that can be error-recovered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErrorRecovery<S, T> {
    Error(Vec<(usize, Token<S>, usize)>),
    Value(T),
}

impl<S, T> ErrorRecovery<S, T> {
    /// Return the value or an error.
    pub fn recover(self) -> Result<T> {
        use self::ErrorRecovery::*;

        match self {
            Error(_) => Err("value not available".into()),
            Value(value) => Ok(value),
        }
    }
}

impl<S, T> From<T> for ErrorRecovery<S, T> {
    fn from(value: T) -> ErrorRecovery<S, T> {
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
pub struct Item<S, T> {
    pub comment: Vec<S>,
    pub attributes: Vec<Loc<Attribute<S>>>,
    pub item: Loc<T>,
}

/// Item derefs into target.
impl<S, T> ops::Deref for Item<S, T> {
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
pub enum AttributeItem<S> {
    Word(Loc<Value<S>>),
    NameValue { name: Loc<S>, value: Loc<Value<S>> },
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
pub enum Attribute<S> {
    Word(Loc<S>),
    List(Loc<S>, Vec<AttributeItem<S>>),
}

/// A type.
///
/// For example: `u32`, `::Relative::Name`, or `bytes`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type<S> {
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
        name: Name<S>,
    },
    Array {
        inner: Box<Type<S>>,
    },
    Map {
        key: Box<Type<S>>,
        value: Box<Type<S>>,
    },
}

/// Any kind of declaration.
#[derive(Debug, PartialEq, Eq)]
pub enum Decl<S> {
    Type(Item<S, TypeBody<S>>),
    Tuple(Item<S, TupleBody<S>>),
    Interface(Item<S, InterfaceBody<S>>),
    Enum(Item<S, EnumBody<S>>),
    Service(Item<S, ServiceBody<S>>),
}

impl<S> Decl<S> {
    /// Get the local name for the declaration.
    pub fn name(&self) -> &S {
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
    pub fn decls(&self) -> Decls<S> {
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

pub struct Decls<'a, S: 'a> {
    iter: vec::IntoIter<&'a Decl<S>>,
}

impl<'a, S: 'a> Iterator for Decls<'a, S> {
    type Item = &'a Decl<S>;

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
pub struct EnumBody<S> {
    pub name: S,
    pub ty: Loc<Type<S>>,
    pub variants: Vec<Item<S, EnumVariant<S>>>,
    pub members: Vec<EnumMember<S>>,
}

impl<S> EnumBody<S> {
    /// Access all inner declarations.
    fn decls(&self) -> Vec<&Decl<S>> {
        Vec::new()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct EnumVariant<S> {
    pub name: Loc<S>,
    pub argument: Option<Loc<Value<S>>>,
}

/// A member in a tuple, type, or interface.
#[derive(Debug, PartialEq, Eq)]
pub enum EnumMember<S> {
    Code(Loc<Code<S>>),
}

/// A field.
///
/// ```ignore
/// <name><modifier>: <ty> as <field_as>
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct Field<S> {
    pub required: bool,
    pub name: S,
    pub ty: Loc<ErrorRecovery<S, Type<S>>>,
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
pub struct File<S> {
    pub comment: Vec<S>,
    pub attributes: Vec<Loc<Attribute<S>>>,
    pub uses: Vec<Loc<UseDecl<S>>>,
    pub decls: Vec<Decl<S>>,
}

impl<S> Field<S> {
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
pub enum Name<S> {
    Relative {
        parts: Vec<Loc<S>>,
    },
    Absolute {
        prefix: Option<Loc<S>>,
        parts: Vec<Loc<S>>,
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
pub struct InterfaceBody<S> {
    pub name: S,
    pub members: Vec<TypeMember<S>>,
    pub sub_types: Vec<Item<S, SubType<S>>>,
}

impl<S> InterfaceBody<S> {
    /// Access all inner declarations.
    fn decls(&self) -> Vec<&Decl<S>> {
        let mut out = Vec::new();

        for m in &self.members {
            if let TypeMember::InnerDecl(ref decl) = *m {
                out.push(decl);
            }
        }

        out
    }

    /// Access all fields.
    pub fn fields(&self) -> Vec<&Field<S>> {
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
pub struct Code<S> {
    pub attributes: Vec<Loc<Attribute<S>>>,
    pub context: Loc<S>,
    pub content: Vec<S>,
}

/// A member in a tuple, type, or interface.
#[derive(Debug, PartialEq, Eq)]
pub enum TypeMember<S> {
    Field(Item<S, Field<S>>),
    Code(Loc<Code<S>>),
    InnerDecl(Decl<S>),
}

/// The body of a service declaration.
///
/// ```ignore
/// service <name> {
///   <members>
/// }
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct ServiceBody<S> {
    pub name: S,
    pub members: Vec<ServiceMember<S>>,
}

impl<S> ServiceBody<S> {
    /// Access all inner declarations.
    fn decls(&self) -> Vec<&Decl<S>> {
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
pub enum ServiceMember<S> {
    Endpoint(Item<S, Endpoint<S>>),
    InnerDecl(Decl<S>),
}

/// The argument in and endpoint.
#[derive(Debug, PartialEq, Eq)]
pub struct EndpointArgument<S> {
    pub ident: Loc<S>,
    pub channel: Loc<Channel<S>>,
}

/// An endpoint
///
/// ```ignore
/// <id>(<arguments>) -> <response> as <alias> {
///   <options>
/// }
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct Endpoint<S> {
    pub id: Loc<S>,
    pub alias: Option<String>,
    pub arguments: Vec<EndpointArgument<S>>,
    pub response: Option<Loc<Channel<S>>>,
}

/// Describes how data is transferred over a channel.
///
/// ```ignore
/// Unary(stream <ty>)
/// Streaming(<ty>)
/// ```
#[derive(Debug, PartialEq, Eq)]
pub enum Channel<S> {
    /// Single send.
    Unary { ty: Type<S> },
    /// Multiple sends.
    Streaming { ty: Type<S> },
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
pub struct SubType<S> {
    pub name: Loc<S>,
    pub members: Vec<TypeMember<S>>,
    pub alias: Option<Loc<Value<S>>>,
}

/// The body of a tuple
///
/// ```ignore
/// tuple <name> {
///     <members>
/// }
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct TupleBody<S> {
    pub name: S,
    pub members: Vec<TypeMember<S>>,
}

impl<S> TupleBody<S> {
    /// Access all inner declarations.
    fn decls(&self) -> Vec<&Decl<S>> {
        let mut out = Vec::new();

        for m in &self.members {
            if let TypeMember::InnerDecl(ref decl) = *m {
                out.push(decl);
            }
        }

        out
    }

    /// Access all fields.
    pub fn fields(&self) -> Vec<&Field<S>> {
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
pub struct TypeBody<S> {
    pub name: S,
    pub members: Vec<TypeMember<S>>,
}

impl<S> TypeBody<S> {
    /// Access all inner declarations.
    fn decls(&self) -> Vec<&Decl<S>> {
        let mut out = Vec::new();

        for m in &self.members {
            if let TypeMember::InnerDecl(ref decl) = *m {
                out.push(decl);
            }
        }

        out
    }

    /// Access all fields.
    pub fn fields(&self) -> Vec<&Field<S>> {
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
pub struct UseDecl<S> {
    pub package: Loc<RpPackage>,
    pub range: Option<Loc<String>>,
    pub alias: Option<Loc<S>>,
}

/// A literal value
///
/// For example, `"string"`, `42.0`, and `foo`.
#[derive(Debug, PartialEq, Eq)]
pub enum Value<S> {
    String(String),
    Number(RpNumber),
    Identifier(S),
    Array(Vec<Loc<Value<S>>>),
}

/// A part of a step.
#[derive(Debug, PartialEq, Eq)]
pub enum PathPart<S> {
    Variable(S),
    Segment(String),
}

/// A step in a path specification.
#[derive(Debug, PartialEq, Eq)]
pub struct PathStep<S> {
    pub parts: Vec<PathPart<S>>,
}

/// A path specification.
#[derive(Debug, PartialEq, Eq)]
pub struct PathSpec<S> {
    pub steps: Vec<PathStep<S>>,
}
