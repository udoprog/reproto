use core::{RpName, RpType, RpVersionedPackage};
use core::errors::*;
use genco::{Cons, Csharp, IntoTokens, Quoted, Tokens};
use genco::csharp::{self, array, struct_, using};
use processor::Processor;
use std::rc::Rc;
use trans::Environment;

pub struct Utils {
    env: Rc<Environment>,
    list: Csharp<'static>,
    dictionary: Csharp<'static>,
    string: Csharp<'static>,
    date_time: Csharp<'static>,
    object: Csharp<'static>,
    pub void: Csharp<'static>,
}

impl Processor for Utils {}

impl Utils {
    pub fn new(env: &Rc<Environment>) -> Utils {
        Utils {
            env: Rc::clone(env),
            list: using("System.Collections.Generic", "List"),
            dictionary: using("System.Collections.Generic", "Dictionary"),
            string: using("System", "String"),
            date_time: struct_(using("System", "DateTime")),
            object: using("System", "Object"),
            void: using("java.lang", "Void"),
        }
    }

    /// Convert the given type to a java type.
    pub fn into_csharp_type<'el>(&self, ty: &RpType) -> Result<Csharp<'el>> {
        use self::RpType::*;

        let out = match *ty {
            String => self.string.clone().into(),
            DateTime => self.date_time.clone().into(),
            Signed { size: 32 } => csharp::INT32.into(),
            Signed { size: 64 } => csharp::INT64.into(),
            Unsigned { size: 32 } => csharp::UINT32.into(),
            Unsigned { size: 64 } => csharp::UINT64.into(),
            Float => csharp::SINGLE.into(),
            Double => csharp::DOUBLE.into(),
            Boolean => csharp::BOOLEAN.into(),
            Array { ref inner } => {
                let argument = self.into_csharp_type(inner)?;
                self.list.with_arguments(vec![argument]).into()
            }
            Name { ref name } => self.convert_type_id(name)?,
            Map { ref key, ref value } => {
                let key = self.into_csharp_type(key)?;
                let value = self.into_csharp_type(value)?;
                self.dictionary.with_arguments(vec![key, value]).into()
            }
            Any => self.object.clone().into(),
            Bytes => array(csharp::BYTE),
            _ => return Err(format!("unsupported type: {}", ty).into()),
        };

        Ok(out)
    }

    pub fn convert_type_id<'b, 'el>(&self, name: &'b RpName) -> Result<Csharp<'el>> {
        let registered = self.env.lookup(name)?;

        let package_name = self.csharp_package_name(&name.package);

        let name = Rc::new(registered.ident(name, |p| p.join("."), |c| c.join(".")));

        let ty = using(package_name, name);

        if registered.is_enum() {
            return Ok(ty.into_enum());
        }

        Ok(ty)
    }

    fn csharp_package_name(&self, pkg: &RpVersionedPackage) -> Rc<String> {
        Rc::new(self.csharp_package(pkg).parts.join("."))
    }
}

/// [DataMember(..)] attribute
#[allow(unused)]
pub struct DataMember<'el> {
    name: Cons<'el>,
    emit_default_value: bool,
}

impl<'el> DataMember<'el> {
    /// Create a new `DataMember` attributes.
    #[allow(unused)]
    pub fn new(name: Cons<'el>) -> DataMember {
        DataMember {
            name: name,
            emit_default_value: false,
        }
    }
}

impl<'el> IntoTokens<'el, Csharp<'el>> for DataMember<'el> {
    fn into_tokens(self) -> Tokens<'el, Csharp<'el>> {
        let data_member = using("System.Runtime.Serialization", "DataMember");

        let mut args: Tokens<'el, Csharp<'el>> = Tokens::new();
        args.append(toks!["Name = ", self.name.quoted()]);
        args.append(toks![
            "EmitDefaultValue = ",
            self.emit_default_value.to_string(),
        ]);

        toks!["[", data_member, "(", args.join(", "), ")]"]
    }
}
