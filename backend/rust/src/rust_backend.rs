use super::*;
use std::rc::Rc;

const TYPE_SEP: &'static str = "_";
const SCOPE_SEP: &'static str = "::";

pub struct RustBackend {
    pub env: Environment,
    listeners: Box<Listeners>,
    id_converter: Option<Box<Naming>>,
    to_lower_snake: Box<Naming>,
    hash_map: ImportedName,
    json_value: ImportedName,
}

impl RustBackend {
    pub fn new(
        env: Environment,
        _: RustOptions,
        listeners: Box<Listeners>,
        id_converter: Option<Box<Naming>>,
    ) -> RustBackend {
        RustBackend {
            env: env,
            listeners: listeners,
            id_converter: id_converter,
            to_lower_snake: SnakeCase::new().to_lower_snake(),
            hash_map: Name::imported("std::collections", "HashMap"),
            json_value: Name::imported_alias("serde_json", "Value", "json"),
        }
    }

    pub fn compiler(&self, options: CompilerOptions) -> Result<RustCompiler> {
        Ok(RustCompiler {
            out_path: options.out_path,
            backend: self,
        })
    }

    pub fn verify(&self) -> Result<()> {
        Ok(())
    }

    fn ident(&self, name: &str) -> String {
        if let Some(ref id_converter) = self.id_converter {
            id_converter.convert(name)
        } else {
            name.to_owned()
        }
    }

    fn convert_type_name(&self, name: &RpName) -> String {
        name.join(TYPE_SEP)
    }

    fn convert_type_id(&self, name: &RpName) -> Result<Name> {
        let registered = self.env.lookup(name)?;

        let local_name = registered.local_name(&name, |p| p.join(TYPE_SEP), |c| c.join(SCOPE_SEP));

        if let Some(ref prefix) = name.prefix {
            let package_name = self.package(&name.package).parts.join("::");
            return Ok(Name::Imported(
                Name::imported_alias(&package_name, &local_name, prefix),
            ));
        }

        Ok(Name::Local(Name::local(&local_name)))
    }

    fn into_type(&self, name: &RpName, field: &Loc<RpField>) -> Result<Statement> {
        let stmt = self.into_rust_type(name, &field.ty).with_pos(field.pos())?;

        if field.is_optional() {
            return Ok(stmt!["Option<", stmt, ">"]);
        }

        Ok(stmt)
    }

    pub fn into_rust_type(&self, name: &RpName, ty: &RpType) -> Result<Statement> {
        let ty = match *ty {
            RpType::String => stmt!["String"],
            RpType::Signed { ref size } => {
                if size.map(|s| s <= 32usize).unwrap_or(true) {
                    stmt!["i32"]
                } else {
                    stmt!["i64"]
                }
            }
            RpType::Unsigned { ref size } => {
                if size.map(|s| s <= 32usize).unwrap_or(true) {
                    stmt!["u32"]
                } else {
                    stmt!["u64"]
                }
            }
            RpType::Float => stmt!["f32"],
            RpType::Double => stmt!["f64"],
            RpType::Boolean => stmt!["bool"],
            RpType::Array { ref inner } => {
                let argument = self.into_rust_type(name, inner)?;
                stmt!["Vec<", argument, ">"]
            }
            RpType::Name { ref name } => stmt![self.convert_type_id(name)?],
            RpType::Map { ref key, ref value } => {
                let key = self.into_rust_type(name, key)?;
                let value = self.into_rust_type(name, value)?;
                stmt![&self.hash_map, "<", key, ", ", value, ">"]
            }
            RpType::Any => stmt![&self.json_value],
            ref t => {
                return Err(format!("unsupported type: {:?}", t).into());
            }
        };

        Ok(ty)
    }

    // Build the corresponding element out of a field declaration.
    fn field_element(&self, name: &RpName, field: &Loc<RpField>) -> Result<Element> {
        let mut elements = Elements::new();

        let ident = self.ident(field.ident());
        let type_spec = self.into_type(name, field)?;

        if field.is_optional() {
            elements.push(stmt!["#[serde(skip_serializing_if=\"Option::is_none\")]"]);
        }

        if field.name() != ident {
            elements.push(stmt![
                "#[serde(rename = ",
                Variable::String(field.name().to_owned()),
                ")]",
            ]);
        }

        elements.push(stmt![ident, ": ", type_spec, ","]);

        Ok(elements.into())
    }

    pub fn process_tuple(
        &self,
        out: &mut RustFileSpec,
        name: &RpName,
        body: Rc<Loc<RpTupleBody>>,
    ) -> Result<()> {
        let mut fields = Statement::new();

        for field in &body.fields {
            fields.push(self.into_type(name, field)?);
        }

        let name = self.convert_type_name(name);

        let mut elements = Elements::new();
        elements.push("#[derive(Serialize, Deserialize, Debug)]");
        elements.push(stmt!["struct ", &name, "(", fields.join(", "), ");"]);

        out.0.push(elements);
        Ok(())
    }

    pub fn process_enum(
        &self,
        out: &mut RustFileSpec,
        name: &RpName,
        body: Rc<Loc<RpEnumBody>>,
    ) -> Result<()> {
        let name = self.convert_type_name(name);
        let mut enum_spec = EnumSpec::new(&name);
        enum_spec.public();

        for code in body.codes.for_context(RUST_CONTEXT) {
            enum_spec.push(code.take().lines);
        }

        let mut elements = Elements::new();

        elements.push(stmt![body.local_name.as_str(), " {"]);

        body.variants.for_each_loc(|variant| {
            if let RpEnumOrdinal::String(ref s) = variant.ordinal {
                if s != variant.local_name.value() {
                    let rename = stmt!["#[serde(rename = ", Variable::String(s.to_owned()), ")]"];
                    elements.push_nested(rename);
                }
            }

            elements.push_nested(stmt![variant.local_name.value(), ","]);
            Ok(()) as Result<()>
        })?;

        elements.push("}");

        enum_spec.push(elements);

        out.0.push(enum_spec);
        Ok(())
    }

    pub fn process_type(
        &self,
        out: &mut RustFileSpec,
        name: &RpName,
        body: Rc<Loc<RpTypeBody>>,
    ) -> Result<()> {
        let mut fields = Elements::new();

        for field in &body.fields {
            fields.push(self.field_element(name, field)?);
        }

        let name = self.convert_type_name(name);
        let mut struct_spec = StructSpec::new(&name);
        struct_spec.public();

        struct_spec.push_attribute("#[derive(Serialize, Deserialize, Debug)]");
        struct_spec.push(fields);

        for code in body.codes.for_context(RUST_CONTEXT) {
            struct_spec.push(code.take().lines);
        }

        out.0.push(struct_spec);
        Ok(())
    }

    pub fn process_interface(
        &self,
        out: &mut RustFileSpec,
        name: &RpName,
        body: Rc<Loc<RpInterfaceBody>>,
    ) -> Result<()> {
        let type_name = self.convert_type_name(name);
        let mut enum_spec = EnumSpec::new(&type_name);
        enum_spec.public();

        enum_spec.push_attribute("#[derive(Serialize, Deserialize, Debug)]");
        enum_spec.push_attribute("#[serde(tag = \"type\")]");

        for code in body.codes.for_context(RUST_CONTEXT) {
            enum_spec.push(code.take().lines);
        }

        let sub_types = body.sub_types.values().map(AsRef::as_ref);

        sub_types.for_each_loc(|sub_type| {
            let mut elements = Elements::new();

            if let Some(sub_type_name) = sub_type.names.first() {
                elements.push(stmt![
                    "#[serde(rename = ",
                    Variable::String((**sub_type_name).to_owned()),
                    ")]",
                ]);
            }

            elements.push(stmt![sub_type.local_name.as_str(), " {"]);

            for field in body.fields.iter().chain(sub_type.fields.iter()) {
                elements.push_nested(self.field_element(name, field)?);
            }

            elements.push("},");

            enum_spec.push(elements);

            Ok(()) as Result<()>
        })?;

        out.0.push(enum_spec);

        Ok(())
    }
}

impl PackageUtils for RustBackend {}
