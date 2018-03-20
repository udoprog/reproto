use core::{RpName, RpType, RpVersionedPackage};
use core::errors::*;
use genco::{IntoTokens, Java, Tokens};
use genco::java::{BOOLEAN, DOUBLE, FLOAT, INTEGER, LONG, imported};
use processor::Processor;
use std::rc::Rc;
use trans::Environment;

pub struct Utils {
    env: Rc<Environment>,
    list: Java<'static>,
    map: Java<'static>,
    string: Java<'static>,
    instant: Java<'static>,
    object: Java<'static>,
    byte_buffer: Java<'static>,
    pub void: Java<'static>,
}

impl Processor for Utils {}

impl Utils {
    pub fn new(env: &Rc<Environment>) -> Utils {
        Utils {
            env: Rc::clone(env),
            list: imported("java.util", "List"),
            map: imported("java.util", "Map"),
            string: imported("java.lang", "String"),
            instant: imported("java.time", "Instant"),
            object: imported("java.lang", "Object"),
            byte_buffer: imported("java.nio", "ByteBuffer"),
            void: imported("java.lang", "Void"),
        }
    }

    /// Convert the given type to a java type.
    pub fn into_java_type<'el>(&self, ty: &RpType) -> Result<Java<'el>> {
        use self::RpType::*;

        let out = match *ty {
            String => self.string.clone().into(),
            DateTime => self.instant.clone().into(),
            Signed { size: 32 } => INTEGER.into(),
            Signed { size: 64 } => LONG.into(),
            Unsigned { size: 32 } => INTEGER.into(),
            Unsigned { size: 64 } => LONG.into(),
            Float => FLOAT.into(),
            Double => DOUBLE.into(),
            Boolean => BOOLEAN.into(),
            Array { ref inner } => {
                let argument = self.into_java_type(inner)?;
                self.list.with_arguments(vec![argument]).into()
            }
            Name { ref name } => self.convert_type_id(name)?,
            Map { ref key, ref value } => {
                let key = self.into_java_type(key)?;
                let value = self.into_java_type(value)?;
                self.map.with_arguments(vec![key, value]).into()
            }
            Any => self.object.clone().into(),
            Bytes => self.byte_buffer.clone().into(),
            _ => return Err(format!("unsupported type: {}", ty).into()),
        };

        Ok(out)
    }

    pub fn convert_type_id<'b, 'el>(&self, name: &'b RpName) -> Result<Java<'el>> {
        let registered = self.env.lookup(name)?;

        let package_name = self.java_package_name(&name.package);

        let name = Rc::new(registered.ident(name, |p| p.join("."), |c| c.join(".")));

        Ok(imported(package_name, name))
    }

    fn java_package_name(&self, pkg: &RpVersionedPackage) -> Rc<String> {
        Rc::new(self.java_package(pkg).parts.join("."))
    }
}

/// @Override annotation
pub struct Override;

impl<'el> IntoTokens<'el, Java<'el>> for Override {
    fn into_tokens(self) -> Tokens<'el, Java<'el>> {
        toks!["@Override"]
    }
}
