//! # Helper trait to deal with value construction
//!
//! RpValue construction is when a literal value is encoded into the output.
//!
//! For example, when creating an instance of type `Foo(1, 2, 3)` in java could be translated to:
//!
//! ```java
//! new Foo(1, 2F, 3D)
//! ```
//!
//! In this example, the second field is a `float`, and the third field is a `double`.

use core::*;
use std::collections::HashMap;
use super::converter::Converter;
use super::environment::Environment;
use super::errors::*;
use super::variables::*;

pub type KnownValues<T> = HashMap<String, T>;

pub struct ValueBuilderEnv<'a> {
    pub package: &'a RpPackage,
    pub variables: &'a Variables<'a>,
    pub value: &'a RpLoc<RpValue>,
    pub ty: Option<&'a RpType>,
}

fn new_env<'a>(package: &'a RpPackage,
               variables: &'a Variables,
               value: &'a RpLoc<RpValue>,
               ty: Option<&'a RpType>)
               -> Box<ValueBuilderEnv<'a>> {
    Box::new(ValueBuilderEnv {
        package: package,
        variables: variables,
        value: value,
        ty: ty,
    })
}

pub trait ValueBuilder
    where Self: Converter
{
    fn env(&self) -> &Environment;

    fn signed(&self, number: &RpNumber, _: &Option<usize>) -> Result<Self::Stmt> {
        self.number(number)
    }

    fn unsigned(&self, number: &RpNumber, _: &Option<usize>) -> Result<Self::Stmt> {
        self.number(number)
    }

    fn float(&self, number: &RpNumber) -> Result<Self::Stmt> {
        self.number(number)
    }

    fn double(&self, number: &RpNumber) -> Result<Self::Stmt> {
        self.number(number)
    }

    fn string(&self, &str) -> Result<Self::Stmt>;

    fn boolean(&self, &bool) -> Result<Self::Stmt>;

    fn number(&self, &RpNumber) -> Result<Self::Stmt>;

    fn array(&self, values: Vec<Self::Stmt>) -> Result<Self::Stmt>;

    fn optional_empty(&self) -> Result<Self::Stmt>;

    fn constant(&self, ty: Self::Type) -> Result<Self::Stmt>;

    fn instance(&self, ty: Self::Type, arguments: Vec<Self::Stmt>) -> Result<Self::Stmt>;

    fn identifier(&self, identifier: &str) -> Result<Self::Stmt>;

    fn value(&self, env: &ValueBuilderEnv) -> Result<Self::Stmt> {
        let value = env.value;
        let ty = env.ty;

        match (&**value, ty) {
            (&RpValue::String(ref string), Some(&RpType::String)) |
            (&RpValue::String(ref string), None) => {
                return self.string(string);
            }
            (&RpValue::Boolean(ref boolean), Some(&RpType::Boolean)) |
            (&RpValue::Boolean(ref boolean), None) => {
                return self.boolean(boolean);
            }
            (&RpValue::Number(ref number), None) => {
                return self.number(number);
            }
            (&RpValue::Number(ref number), Some(&RpType::Signed(ref size))) => {
                return self.signed(number, size);
            }
            (&RpValue::Number(ref number), Some(&RpType::Unsigned(ref size))) => {
                return self.unsigned(number, size);
            }
            (&RpValue::Number(ref number), Some(&RpType::Float)) => {
                return self.float(number);
            }
            (&RpValue::Number(ref number), Some(&RpType::Double)) => {
                return self.double(number);
            }
            (&RpValue::Array(ref values), expected) => {
                let inner = match expected {
                    Some(&RpType::Array(ref inner)) => Some(&**inner),
                    Some(other) => {
                        return Err(Error::pos(format!("expected `{}`", other), value.pos.clone()))
                    }
                    None => None,
                };

                let mut array_values = Vec::new();

                for v in values {
                    let new_env = new_env(&env.package, &env.variables, v, inner);
                    array_values.push(self.value(&*new_env)?)
                }

                return self.array(array_values);
            }
            (&RpValue::Constant(ref constant), Some(&RpType::Name(ref target))) => {
                let reg_constant = self.env()
                    .constant(&value.pos, &env.package, constant, target)?;

                match *reg_constant {
                    RpRegistered::EnumConstant { parent: _, variant: _ } => {
                        let ty =
                            self.convert_constant(&value.pos, &env.package.into_type_id(constant))?;
                        return self.constant(ty);
                    }
                    _ => {
                        return Err(Error::pos("not a valid enum constant".into(),
                                              value.pos.clone()))
                    }
                }
            }
            (&RpValue::Instance(ref instance), Some(&RpType::Name(ref target))) => {
                let (registered, known) = self.env()
                    .instance(&value.pos, env.package, instance, target)?;

                let mut arguments = Vec::new();

                for f in registered.fields()? {
                    if let Some(init) = known.get(&f.name) {
                        let new_env = new_env(env.package, env.variables, &init.value, Some(&f.ty));
                        arguments.push(self.value(&*new_env)?);
                    } else {
                        arguments.push(self.optional_empty()?);
                    }
                }

                let ty = self.convert_type(&value.pos, &env.package.into_type_id(&instance.ty))?;
                return self.instance(ty, arguments);
            }
            // identifier with any type.
            (&RpValue::Identifier(ref identifier), expected) => {
                if let Some(variable_type) = env.variables.get(identifier) {
                    // if expected is set
                    if let Some(expected) = expected {
                        if !self.env().is_assignable_from(&env.package, expected, variable_type)? {
                            return Err(Error::pos(format!("not assignable to `{}`", expected)
                                                      .into(),
                                                  value.pos.clone()));
                        }
                    }

                    return self.identifier(identifier);
                } else {
                    return Err(Error::pos("missing variable".into(), value.pos.clone()));
                }
            }
            _ => {}
        }

        if let Some(ty) = ty {
            Err(Error::pos(format!("expected `{}`", ty), value.pos.clone()))
        } else {
            Err(Error::pos("unexpected value".into(), value.pos.clone()))
        }
    }
}
