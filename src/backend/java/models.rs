pub use backend::errors::*;
pub use backend::models::*;
use codeviz::java::{self, Statement, Modifiers};

/// A single field.
#[derive(Debug, Clone)]
pub struct JavaField {
    pub modifier: Modifier,
    pub camel_name: String,
    pub name: String,
    pub ident: String,
    pub ty: java::Type,
    pub spec: java::FieldSpec,
}

impl JavaField {
    pub fn setter(&self) -> Result<Option<java::MethodSpec>> {
        if self.spec.modifiers.contains(&java::Modifier::Final) {
            return Ok(None);
        }

        let name = format!("set{}", self.camel_name);
        let mut setter = java::MethodSpec::new(mods![java::Modifier::Public], &name);

        let argument = java::ArgumentSpec::new(mods![java::Modifier::Final], &self.ty, &self.ident);

        setter.push_argument(&argument);
        setter.returns(java::VOID);

        let mut method_body = java::Elements::new();

        method_body.push(stmt!["this.", &self.ident, " = ", &argument, ";"]);
        setter.push(method_body);

        Ok(Some(setter))
    }

    pub fn getter(&self) -> Result<java::MethodSpec> {
        let name = format!("get{}", self.camel_name);
        let mut getter = java::MethodSpec::new(mods![java::Modifier::Public], &name);
        getter.returns(&self.ty);
        getter.push(stmt!["return this.", &self.ident, ";"]);
        Ok(getter)
    }
}
