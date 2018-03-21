//! Code generator for the given path.

use {Compiler, Options, Utils};
use core::{Handle, RpEnumBody, RpInterfaceBody, RpServiceBody};
use core::errors::Result;
use genco::{Cons, Java};
use genco::java::{Argument, Class, Enum, Interface, Method};
use std::rc::Rc;

/// Generate helper implementations for codegen traits.
macro_rules! codegen {
    ($type: tt, $e: ty) => {
        impl<T> $type for Rc<T>
        where
            T: $type,
        {
            fn generate(&self, e: $e) -> Result<()> {
                self.as_ref().generate(e)
            }
        }
    };
}

pub struct GetterAdded<'a, 'el: 'a> {
    pub name: Cons<'el>,
    pub getter: &'a mut Method<'el>,
}

pub struct ClassAdded<'a, 'el: 'a> {
    pub names: &'a [Cons<'el>],
    pub spec: &'a mut Class<'el>,
}

pub struct TupleAdded<'a, 'el: 'a> {
    pub spec: &'a mut Class<'el>,
}

pub struct EnumAdded<'a, 'el: 'a> {
    pub body: &'el RpEnumBody,
    pub spec: &'a mut Enum<'el>,
    pub from_value: &'a mut Method<'el>,
    pub to_value: &'a mut Method<'el>,
}

pub struct InterfaceAdded<'a, 'el: 'a> {
    pub body: &'el RpInterfaceBody,
    pub spec: &'a mut Interface<'el>,
}

pub struct EndpointExtra<'el> {
    pub name: Cons<'el>,
    pub response_ty: Java<'el>,
    pub request_ty: Java<'el>,
    pub arguments: Vec<Argument<'el>>,
}

pub struct ServiceAdded<'a, 'el: 'a> {
    pub compiler: &'a Compiler<'el>,
    pub body: &'el RpServiceBody,
    pub extra: &'a [EndpointExtra<'el>],
    pub spec: &'a mut Interface<'el>,
}

pub struct Configure<'a> {
    pub options: &'a mut Options,
    pub utils: &'a Rc<Utils>,
}

pub trait Codegen {
    /// Build the given piece of code in the given handle.
    fn generate(&self, handle: &Handle) -> Result<()>;
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

/// Generate code for getters.
pub trait GetterCodegen {
    fn generate(&self, e: GetterAdded) -> Result<()>;
}

codegen!(GetterCodegen, GetterAdded);

/// Generate class-based code.
pub trait ClassCodegen {
    fn generate(&self, e: ClassAdded) -> Result<()>;
}

codegen!(ClassCodegen, ClassAdded);

/// Generate tuple-based code.
pub trait TupleCodegen {
    fn generate(&self, e: TupleAdded) -> Result<()>;
}

codegen!(TupleCodegen, TupleAdded);

/// Generate interface-based code.
pub trait InterfaceCodegen {
    fn generate(&self, e: InterfaceAdded) -> Result<()>;
}

codegen!(InterfaceCodegen, InterfaceAdded);

/// Generate enum-based code.
pub trait EnumCodegen {
    fn generate(&self, e: EnumAdded) -> Result<()>;
}

codegen!(EnumCodegen, EnumAdded);
