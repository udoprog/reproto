//! Code generator for the given path.

use core::Handle;
use core::errors::Result;
use core::flavored::{RpEnumBody, RpInterfaceBody, RpServiceBody};
use csharp_field::CsharpField;
use genco::csharp::{Argument, Class, Enum, Field};
use genco::{Cons, Csharp};
use std::rc::Rc;
use {Compiler, Options, Utils};

#[derive(Clone)]
pub struct TypeField<'el> {
    pub field: Field<'el>,
    pub tag: Cons<'el>,
}

pub struct ClassAdded<'a, 'el: 'a> {
    /// Type field to register in the class.
    pub type_field: Option<TypeField<'el>>,
    /// Names of all fields in class.
    pub names: &'a [Cons<'el>],
    /// Class specification.
    pub spec: &'a mut Class<'el>,
    /// Fields of the added class.
    pub fields: &'a [CsharpField<'el>],
}

pub struct TupleAdded<'a, 'el: 'a> {
    pub spec: &'a mut Class<'el>,
}

pub struct EnumAdded<'a, 'el: 'a> {
    pub body: &'el RpEnumBody,
    pub spec: &'a mut Enum<'el>,
    pub names: &'a [Cons<'el>],
}

pub struct InterfaceAdded<'a, 'el: 'a> {
    pub body: &'el RpInterfaceBody,
    pub spec: &'a mut Class<'el>,
}

pub struct EndpointExtra<'el> {
    pub name: Cons<'el>,
    pub response_ty: Csharp<'el>,
    pub arguments: Vec<Argument<'el>>,
}

pub struct ServiceAdded<'a, 'el: 'a> {
    pub compiler: &'a Compiler,
    pub body: &'el RpServiceBody,
    pub extra: &'a [EndpointExtra<'el>],
    pub spec: &'a mut Class<'el>,
}

pub struct TypeFieldAdded<'a, 'el: 'a> {
    /// Tag used for the field.
    pub tag: Cons<'el>,
    /// Type field to register in the class.
    pub field: &'a mut Field<'el>,
}

pub struct Configure<'a> {
    pub options: &'a mut Options,
    pub utils: &'a Rc<Utils>,
}

pub trait Codegen {
    /// Build the given piece of code in the given handle.
    fn generate(&self, handle: &Handle) -> Result<()>;
}

impl<T> Codegen for Rc<T>
where
    T: Codegen,
{
    fn generate(&self, handle: &Handle) -> Result<()> {
        self.as_ref().generate(handle)
    }
}

/// Generate service-based code.
pub trait ServiceCodegen {
    fn generate(&self, e: ServiceAdded) -> Result<()>;
}

impl<T> ServiceCodegen for Rc<T>
where
    T: ServiceCodegen,
{
    fn generate(&self, e: ServiceAdded) -> Result<()> {
        self.as_ref().generate(e)
    }
}

/// Generate class-based code.
pub trait ClassCodegen {
    fn generate(&self, e: ClassAdded) -> Result<()>;
}

impl<T> ClassCodegen for Rc<T>
where
    T: ClassCodegen,
{
    fn generate(&self, e: ClassAdded) -> Result<()> {
        self.as_ref().generate(e)
    }
}

/// Generate tuple-based code.
pub trait TupleCodegen {
    fn generate(&self, e: TupleAdded) -> Result<()>;
}

impl<T> TupleCodegen for Rc<T>
where
    T: TupleCodegen,
{
    fn generate(&self, e: TupleAdded) -> Result<()> {
        self.as_ref().generate(e)
    }
}

/// Generate interface-based code.
pub trait InterfaceCodegen {
    fn generate(&self, e: InterfaceAdded) -> Result<()>;
}

impl<T> InterfaceCodegen for Rc<T>
where
    T: InterfaceCodegen,
{
    fn generate(&self, e: InterfaceAdded) -> Result<()> {
        self.as_ref().generate(e)
    }
}

/// Generate enum-based code.
pub trait EnumCodegen {
    fn generate(&self, e: EnumAdded) -> Result<()>;
}

impl<T> EnumCodegen for Rc<T>
where
    T: EnumCodegen,
{
    fn generate(&self, e: EnumAdded) -> Result<()> {
        self.as_ref().generate(e)
    }
}

/// Generate type-field code.
pub trait TypeFieldCodegen {
    fn generate(&self, e: TypeFieldAdded) -> Result<()>;
}

impl<T> TypeFieldCodegen for Rc<T>
where
    T: TypeFieldCodegen,
{
    fn generate(&self, e: TypeFieldAdded) -> Result<()> {
        self.as_ref().generate(e)
    }
}
