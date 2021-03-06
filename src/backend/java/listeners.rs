use backend::errors::*;
use codeviz::java::*;
use super::models as m;
use super::processor::ProcessorOptions;

pub struct ClassAdded<'a> {
    pub fields: &'a Vec<m::JavaField>,
    pub class_type: &'a ClassType,
    pub spec: &'a mut ClassSpec,
}

pub struct TupleAdded<'a> {
    pub fields: &'a Vec<m::JavaField>,
    pub class_type: &'a ClassType,
    pub spec: &'a mut ClassSpec,
}

pub struct EnumAdded<'a> {
    pub body: &'a m::EnumBody,
    pub fields: &'a Vec<m::JavaField>,
    pub class_type: &'a ClassType,
    pub from_value: &'a mut Option<MethodSpec>,
    pub to_value: &'a mut Option<MethodSpec>,
    pub spec: &'a mut EnumSpec,
}

pub struct InterfaceAdded<'a> {
    pub interface: &'a m::InterfaceBody,
    pub spec: &'a mut InterfaceSpec,
}

pub struct SubTypeAdded<'a> {
    pub fields: &'a Vec<m::JavaField>,
    pub interface: &'a m::InterfaceBody,
    pub sub_type: &'a m::SubType,
    pub spec: &'a mut ClassSpec,
}

pub trait Listeners {
    fn configure(&self, _options: &mut ProcessorOptions) -> Result<()> {
        Ok(())
    }

    fn class_added(&self, _: &mut ClassAdded) -> Result<()> {
        Ok(())
    }

    fn tuple_added(&self, _: &mut TupleAdded) -> Result<()> {
        Ok(())
    }

    fn enum_added(&self, _: &mut EnumAdded) -> Result<()> {
        Ok(())
    }

    fn interface_added(&self, _: &mut InterfaceAdded) -> Result<()> {
        Ok(())
    }

    fn sub_type_added(&self, _: &mut SubTypeAdded) -> Result<()> {
        Ok(())
    }
}

/// A vector of listeners is a valid listener.
impl Listeners for Vec<Box<Listeners>> {
    fn configure(&self, processor: &mut ProcessorOptions) -> Result<()> {
        for l in self {
            l.configure(processor)?;
        }

        Ok(())
    }

    fn class_added(&self, event: &mut ClassAdded) -> Result<()> {
        for l in self {
            l.class_added(event)?;
        }

        Ok(())
    }

    fn tuple_added(&self, event: &mut TupleAdded) -> Result<()> {
        for l in self {
            l.tuple_added(event)?;
        }

        Ok(())
    }

    fn enum_added(&self, event: &mut EnumAdded) -> Result<()> {
        for l in self {
            l.enum_added(event)?;
        }

        Ok(())
    }

    fn interface_added(&self, event: &mut InterfaceAdded) -> Result<()> {
        for l in self {
            l.interface_added(event)?;
        }

        Ok(())
    }

    fn sub_type_added(&self, event: &mut SubTypeAdded) -> Result<()> {
        for l in self {
            l.sub_type_added(event)?;
        }

        Ok(())
    }
}
