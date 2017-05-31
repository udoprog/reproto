use backend::*;
use backend::errors::*;
use backend::for_context::ForContext;
use codeviz::js::*;
use naming::{self, FromNaming};
use options::Options;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fs::File;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use super::models::*;
use super::utils::*;

const TYPE: &str = "type";
const EXT: &str = "js";
const JS_CONTEXT: &str = "js";

fn field_ident(_i: usize, field: &JsField) -> Variable {
    string(&field.ident)
}

fn field_index(i: usize, _field: &JsField) -> Variable {
    Variable::Literal(i.to_string())
}

pub trait Listeners {
    fn configure(&self, _processor: &mut ProcessorOptions) -> Result<()> {
        Ok(())
    }
}

/// A vector of listeners is a valid listener.
impl Listeners for Vec<Box<Listeners>> {
    fn configure(&self, processor: &mut ProcessorOptions) -> Result<()> {
        for listeners in self {
            listeners.configure(processor)?;
        }

        Ok(())
    }
}

pub struct ProcessorOptions {
    parent: Options,
    pub build_getters: bool,
    pub build_constructor: bool,
}

impl ProcessorOptions {
    pub fn new(options: Options) -> ProcessorOptions {
        ProcessorOptions {
            parent: options,
            build_getters: false,
            build_constructor: true,
        }
    }
}

pub struct Processor {
    options: ProcessorOptions,
    env: Environment,
    package_prefix: Option<Package>,
    listeners: Box<Listeners>,
    to_lower_snake: Box<naming::Naming>,
    type_var: Variable,
    values: Statement,
    enum_ordinal: Variable,
    enum_name: Variable,
}

impl Processor {
    pub fn new(options: ProcessorOptions,
               env: Environment,
               package_prefix: Option<Package>,
               listeners: Box<Listeners>)
               -> Processor {
        Processor {
            options: options,
            env: env,
            package_prefix: package_prefix,
            listeners: listeners,
            to_lower_snake: naming::SnakeCase::new().to_lower_snake(),
            type_var: string(TYPE),
            values: stmt!["values"],
            enum_ordinal: Variable::Literal("ordinal".to_owned()),
            enum_name: Variable::Literal("name".to_owned()),
        }
    }

    fn find_field<'a>(&self,
                      fields: &'a Vec<Token<JsField>>,
                      name: &str)
                      -> Option<(usize, &'a JsField)> {
        for (i, field) in fields.iter().enumerate() {
            if field.name == name {
                return Some((i, &field.inner));
            }
        }

        None
    }

    /// Build a function that throws an exception if the given value `stmt` is None.
    fn throw_if_null<S>(&self, stmt: S, field: &JsField) -> Elements
        where S: Into<Statement>
    {
        let required_error = string(format!("{}: is a required field", field.name));
        js![if is_not_defined(stmt), js![throw required_error]]
    }

    fn convert_fields(&self, fields: &Vec<Token<Field>>) -> Vec<Token<JsField>> {
        fields.iter()
            .map(|f| {
                let ident = self.field_ident(&f);

                f.clone().map_inner(|o| {
                    JsField {
                        modifier: o.modifier,
                        ty: o.ty,
                        name: f.name().to_owned(),
                        ident: ident,
                    }
                })
            })
            .collect()
    }

    fn encode_method<E, B>(&self,
                           package: &Package,
                           fields: &Vec<Token<JsField>>,
                           builder: B,
                           extra: E)
                           -> Result<MethodSpec>
        where E: FnOnce(&mut Elements) -> (),
              B: Into<Variable>
    {
        let mut encode = MethodSpec::new("encode");
        let mut body = Elements::new();
        let data = stmt!["data"];

        body.push(stmt!["const ", &data, " = ", builder, ";"]);

        extra(&mut body);

        let mut assign = Elements::new();

        for field in fields {
            let var_string = string(field.ident.to_owned());
            let field_stmt = stmt!["this.", &field.ident];
            let value_stmt = self.encode(package, &field.ty, &field_stmt)?;

            match field.modifier {
                Modifier::Optional => {
                    let stmt = js![if is_defined(field_stmt),
                                      stmt![&data, "[", var_string, "] = ", value_stmt, ";"]];
                    assign.push(stmt);
                }
                _ => {
                    assign.push(self.throw_if_null(field_stmt, field));
                    let stmt = stmt![&data, "[", var_string, "] = ", value_stmt, ";"];
                    assign.push(stmt);
                }
            }
        }

        if !assign.is_empty() {
            body.push(assign.join(ElementSpec::Spacing));
        }

        body.push(js![return data]);

        encode.push(body.join(ElementSpec::Spacing));
        Ok(encode)
    }

    fn encode_tuple_method(&self,
                           package: &Package,
                           fields: &Vec<Token<JsField>>)
                           -> Result<MethodSpec> {
        let mut values = Statement::new();

        let mut encode = MethodSpec::new("encode");

        let mut encode_body = Elements::new();

        for field in fields {
            let stmt = stmt!["this.", &field.ident];
            encode_body.push(self.throw_if_null(&stmt, field));
            values.push(self.encode(package, &field.ty, stmt)?);
        }

        encode_body.push(js![@return [ values ]]);
        encode.push(encode_body.join(ElementSpec::Spacing));
        Ok(encode)
    }

    fn encode_enum_method(&self, ident: &str) -> Result<MethodSpec> {
        let mut encode = MethodSpec::new("encode");
        let mut encode_body = Elements::new();
        encode_body.push(js![return "this.", &ident]);
        encode.push(encode_body.join(ElementSpec::Spacing));
        Ok(encode)
    }

    fn decode_enum_method(&self, class: &ClassSpec, ident: &str) -> Result<MethodSpec> {
        let mut decode = MethodSpec::with_static("decode");

        let data = stmt!["data"];
        let i = stmt!["i"];
        let l = stmt!["l"];
        let member = stmt!["member"];

        decode.push_argument(&data);

        let mut member_loop = Elements::new();

        let mut body = Elements::new();

        let members = stmt![&class.name, ".", &self.values];
        body.push(js![const &member, &members, "[", &i, "]"]);

        let cond = stmt![&member, ".", ident, " === ", data];
        body.push(js![if cond, js![return &member]]);

        let loop_init = stmt!["let ", &i, " = 0, ", &l, " = ", &members, ".length"];
        member_loop.push(js![for loop_init; stmt![&i, " < ", &l]; stmt![&i, "++"], body.join(ElementSpec::Spacing)]);

        let mut body = Elements::new();

        body.push(member_loop);
        body.push(js![throw string("no matching value")]);

        decode.push(body.join(ElementSpec::Spacing));
        Ok(decode)
    }

    fn decode_method<F>(&self,
                        package: &Package,
                        fields: &Vec<Token<JsField>>,
                        class: &ClassSpec,
                        variable_fn: F)
                        -> Result<MethodSpec>
        where F: Fn(usize, &JsField) -> Variable
    {
        let mut decode = MethodSpec::with_static("decode");
        let data = stmt!["data"];

        decode.push_argument(&data);

        let mut arguments = Statement::new();
        let mut assign = Elements::new();

        for (i, field) in fields.iter().enumerate() {
            let var_name = field.ident.clone();
            let var = variable_fn(i, field);

            let stmt: ElementSpec = match field.modifier {
                Modifier::Optional => {
                    let var_stmt = self.decode(&field.pos, package, &field.ty, &var_name)?;

                    let mut check = Elements::new();

                    check.push(stmt!["let ", &var_name, " = ", &data, "[", &var, "];"]);
                    check.push(ElementSpec::Spacing);
                    check.push(js![if is_defined(stmt![&var_name]),
                                      stmt![&var_name, " = ", var_stmt, ";"],
                                      stmt![&var_name, " = null", ";"]]);

                    check.into()
                }
                _ => {
                    let var_stmt = stmt![&data, "[", &var, "]"];
                    let var_stmt = self.decode(&field.pos, package, &field.ty, var_stmt)?;
                    stmt!["const ", &var_name, " = ", &var_stmt, ";"].into()
                }
            };

            assign.push(stmt);
            arguments.push(var_name);
        }

        let mut body = Elements::new();

        if !assign.is_empty() {
            body.push(assign.join(ElementSpec::Spacing));
        }

        body.push(js![@return new &class.name, arguments]);

        decode.push(body.join(ElementSpec::Spacing));

        Ok(decode)
    }

    fn is_native(&self, ty: &Type) -> bool {
        match *ty {
            Type::Signed(_) |
            Type::Unsigned(_) => true,
            Type::Float | Type::Double => true,
            Type::String => true,
            Type::Any => true,
            Type::Boolean => true,
            Type::Array(ref inner) => self.is_native(inner),
            _ => false,
        }
    }

    fn field_ident(&self, field: &Field) -> String {
        if let Some(ref id_converter) = self.options.parent.id_converter {
            id_converter.convert(&field.name)
        } else {
            field.name.to_owned()
        }
    }

    fn name(&self,
            pos: &Pos,
            package: &Package,
            used: &Option<String>,
            custom: &Vec<String>)
            -> Result<Name> {
        if let Some(ref used) = *used {
            let package = self.env.lookup_used(pos, package, used)?;
            let package = self.package(package);
            let package = package.parts.join(".");
            Ok(Name::imported_alias(&package, &custom.join("."), used).into())
        } else {
            let package = self.package(package);
            let key = &(package.clone(), custom.to_owned());
            let _ = self.env.types.get(key);
            Ok(Name::local(&custom.join(".")).into())
        }
    }

    fn encode<S>(&self, package: &Package, ty: &Type, value_stmt: S) -> Result<Statement>
        where S: Into<Statement>
    {
        let value_stmt = value_stmt.into();

        // TODO: do not skip conversion if strict type checking is enabled
        if self.is_native(ty) {
            return Ok(value_stmt);
        }

        let value_stmt = match *ty {
            Type::Signed(_) |
            Type::Unsigned(_) => value_stmt,
            Type::Float | Type::Double => value_stmt,
            Type::String => value_stmt,
            Type::Any => value_stmt,
            Type::Boolean => value_stmt,
            Type::Custom(ref _used, ref _custom) => stmt![value_stmt, ".encode()"],
            Type::Array(ref inner) => {
                let v = stmt!["v"];
                let inner = self.encode(package, inner, &v)?;
                stmt![value_stmt, ".map(function(", &v, ") { return ", inner, "; })"]
            }
            _ => value_stmt,
        };

        Ok(value_stmt)
    }

    fn decode<S>(&self, pos: &Pos, package: &Package, ty: &Type, value_stmt: S) -> Result<Statement>
        where S: Into<Statement>
    {
        let value_stmt = value_stmt.into();

        // TODO: do not skip conversion if strict type checking is enabled
        if self.is_native(ty) {
            return Ok(value_stmt);
        }

        let value_stmt = match *ty {
            Type::Signed(_) |
            Type::Unsigned(_) => value_stmt,
            Type::Float | Type::Double => value_stmt,
            Type::String => value_stmt,
            Type::Any => value_stmt,
            Type::Boolean => value_stmt,
            Type::Custom(ref used, ref custom) => {
                let name = self.name(pos, package, used, custom)?;
                stmt![name, ".decode(", value_stmt, ")"]
            }
            Type::Array(ref inner) => {
                let inner = self.decode(pos, package, inner, stmt!["v"])?;
                stmt![value_stmt, ".map(function(v) { ", inner, "; })"]
            }
            _ => value_stmt,
        };

        Ok(value_stmt)
    }


    /// Build the java package of a given package.
    ///
    /// This includes the prefixed configured in `self.options`, if specified.
    fn package(&self, package: &Package) -> Package {
        self.package_prefix
            .clone()
            .map(|prefix| prefix.join(package))
            .unwrap_or_else(|| package.clone())
    }

    fn build_constructor(&self, fields: &Vec<Token<JsField>>) -> ConstructorSpec {
        let mut ctor = ConstructorSpec::new();
        let mut assignments = Elements::new();

        for field in fields {
            ctor.push_argument(stmt![&field.ident]);
            assignments.push(stmt!["this.", &field.ident, " = ", &field.ident, ";"]);
        }

        ctor.push(assignments);
        ctor
    }

    fn build_enum_constructor(&self, fields: &Vec<Token<JsField>>) -> ConstructorSpec {
        let mut ctor = ConstructorSpec::new();
        let mut assignments = Elements::new();

        ctor.push_argument(&self.enum_ordinal);
        assignments.push(stmt!["this.", &self.enum_ordinal, " = ", &self.enum_ordinal, ";"]);

        ctor.push_argument(&self.enum_name);
        assignments.push(stmt!["this.", &self.enum_name, " = ", &self.enum_name, ";"]);

        for field in fields {
            ctor.push_argument(stmt![&field.ident]);
            assignments.push(stmt!["this.", &field.ident, " = ", &field.ident, ";"]);
        }

        ctor.push(assignments);
        ctor
    }

    fn process_tuple(&self, package: &Package, body: &TupleBody) -> Result<ElementSpec> {
        let mut class = ClassSpec::new(&body.name);
        let mut fields: Vec<Token<JsField>> = Vec::new();

        for field in &body.fields {
            let ident = self.field_ident(&field);

            fields.push(field.clone()
                .map_inner(|f| {
                    JsField {
                        modifier: Modifier::Required,
                        ty: f.ty,
                        name: field.name().to_owned(),
                        ident: ident,
                    }
                }));
        }

        class.push(self.build_constructor(&fields));

        // TODO: make configurable
        if false {
            for getter in self.build_getters(&fields)? {
                class.push(&getter);
            }
        }

        let decode = self.decode_method(package, &fields, &class, field_index)?;
        class.push(decode);

        let encode = self.encode_tuple_method(package, &fields)?;
        class.push(encode);

        for code in body.codes.for_context(JS_CONTEXT) {
            class.push(code.inner.lines);
        }

        Ok(class.into())
    }

    fn literal_value(&self, pos: &Pos, value: &Value, ty: &Type) -> Result<Variable> {
        match *ty {
            Type::Double |
            Type::Float |
            Type::Signed(_) |
            Type::Unsigned(_) |
            Type::Boolean => {
                if let Value::Boolean(ref boolean) = *value {
                    return Ok(Variable::Literal(boolean.to_string()));
                }

                if let Value::Number(ref number) = *value {
                    return Ok(Variable::Literal(number.to_string()));
                }
            }
            Type::String => {
                if let Value::String(ref s) = *value {
                    return Ok(string(s));
                }
            }
            _ => {}
        }

        Err(Error::pos(format!("{} cannot be applied to expected type {}", value, ty),
                       pos.clone()))
    }

    fn enum_encode_decode(&self,
                          body: &EnumBody,
                          fields: &Vec<Token<JsField>>,
                          class: &ClassSpec)
                          -> Result<ElementSpec> {
        // lookup serialized_as if specified.
        if let Some(ref s) = body.serialized_as {
            let mut elements = Elements::new();

            if let Some((_, ref field)) = self.find_field(fields, &s.inner) {
                elements.push(self.encode_enum_method(&field.name)?);
                let decode = self.decode_enum_method(&class, &field.name)?;
                elements.push(decode);
                return Ok(elements.into());
            }

            return Err(Error::pos(format!("no field named: {}", s.inner), s.pos.clone()));
        }

        if body.serialized_as_name {
            let mut elements = Elements::new();

            elements.push(self.encode_enum_method("name")?);
            let decode = self.decode_enum_method(&class, "name")?;
            elements.push(decode);
            return Ok(elements.into());
        }

        let mut elements = Elements::new();
        elements.push(self.encode_enum_method("ordinal")?);
        let decode = self.decode_enum_method(&class, "ordinal")?;
        elements.push(decode);
        Ok(elements.into())
    }

    fn process_enum(&self, _package: &Package, body: &EnumBody) -> Result<ElementSpec> {
        let mut class = ClassSpec::new(&body.name);
        let mut fields: Vec<Token<JsField>> = Vec::new();

        for field in &body.fields {
            let ident = self.field_ident(&field);

            let ident = match ident.as_str() {
                "name" => "_name".to_owned(),
                "ordinal" => "_ordinal".to_owned(),
                ident => ident.to_owned(),
            };

            fields.push(field.clone()
                .map_inner(|f| {
                    JsField {
                        modifier: Modifier::Required,
                        ty: f.ty,
                        name: field.name().to_owned(),
                        ident: ident,
                    }
                }));
        }

        let mut members = Statement::new();

        class.push(self.build_enum_constructor(&fields));
        let encode_decode = self.enum_encode_decode(&body, &fields, &class)?;
        class.push(encode_decode);

        let mut values = Elements::new();

        for value in &body.values {
            let mut value_arguments = Statement::new();

            value_arguments.push(value.ordinal.to_string());
            value_arguments.push(string(&value.name));

            for (value, field) in value.arguments.iter().zip(fields.iter()) {
                value_arguments.push(self.literal_value(&value.pos, value, &field.ty)?);
            }

            let arguments = js![new &body.name, value_arguments];
            let member = stmt![&class.name, ".", &value.name];

            values.push(js![= &member, arguments]);
            members.push(member);
        }

        for code in body.codes.for_context(JS_CONTEXT) {
            class.push(code.inner.lines);
        }

        let mut elements = Elements::new();

        // class declaration
        elements.push(&class);

        // enum literal values
        elements.push(values);

        // push members field
        let members_key = stmt![&class.name, ".", &self.values];
        elements.push(js![= members_key, js!([members])]);

        Ok(elements.join(ElementSpec::Spacing).into())
    }

    fn build_getters(&self, fields: &Vec<Token<JsField>>) -> Result<Vec<MethodSpec>> {
        let mut result = Vec::new();

        for field in fields {
            let name = self.to_lower_snake.convert(&field.ident);
            let getter_name = format!("get_{}", name);
            let mut method_spec = MethodSpec::new(&getter_name);
            method_spec.push(js![return "this.", name]);
            result.push(method_spec);
        }

        Ok(result)
    }

    fn process_type(&self, package: &Package, body: &TypeBody) -> Result<ElementSpec> {
        let fields = self.convert_fields(&body.fields);

        let mut class = ClassSpec::new(&body.name);

        let constructor = self.build_constructor(&fields);
        class.push(&constructor);

        // TODO: make configurable
        if false {
            for getter in self.build_getters(&fields)? {
                class.push(getter);
            }
        }

        let decode = self.decode_method(package, &fields, &class, field_ident)?;
        class.push(decode);

        let encode = self.encode_method(package, &fields, "{}", |_| {})?;
        class.push(encode);

        for code in body.codes.for_context(JS_CONTEXT) {
            class.push(code.inner.lines);
        }

        Ok(class.into())
    }

    fn process_interface(&self, package: &Package, body: &InterfaceBody) -> Result<ElementSpec> {
        let mut classes = Elements::new();

        let mut interface_spec = ClassSpec::new(&body.name);

        interface_spec.push(self.interface_decode_method(body)?);

        let interface_fields = self.convert_fields(&body.fields);

        for code in body.codes.for_context(JS_CONTEXT) {
            interface_spec.push(code.inner.lines);
        }

        classes.push(interface_spec);

        for (_, ref sub_type) in &body.sub_types {
            let mut class = ClassSpec::new(&sub_type.name);

            let mut fields = interface_fields.clone();
            fields.extend(self.convert_fields(&sub_type.fields));

            let constructor = self.build_constructor(&fields);
            class.push(&constructor);

            // TODO: make configurable
            if false {
                for getter in self.build_getters(&fields)? {
                    class.push(&getter);
                }
            }

            let decode = self.decode_method(package, &fields, &class, field_ident)?;

            class.push(decode);

            let type_stmt = stmt!["data[", &self.type_var, "] = ", &class.name, ".TYPE;"];

            let encode = self.encode_method(package, &fields, "{}", move |elements| {
                    elements.push(type_stmt);
                })?;

            class.push(encode);

            for code in sub_type.codes.for_context(JS_CONTEXT) {
                class.push(code.inner.lines);
            }

            classes.push(&class);
            classes.push(stmt![&class.name, ".TYPE", " = ", string(sub_type.name.clone()), ";"]);
        }

        Ok(classes.join(ElementSpec::Spacing).into())
    }

    fn populate_files(&self) -> Result<HashMap<&Package, FileSpec>> {
        let mut files = HashMap::new();

        // Process all types discovered so far.
        for (&(ref package, _), decl) in &self.env.types {
            let spec = match decl.inner {
                Decl::Interface(ref body) => self.process_interface(package, body)?,
                Decl::Type(ref body) => self.process_type(package, body)?,
                Decl::Tuple(ref body) => self.process_tuple(package, body)?,
                Decl::Enum(ref body) => self.process_enum(package, body)?,
            };

            match files.entry(package) {
                Entry::Vacant(entry) => {
                    let mut file_spec = FileSpec::new();
                    file_spec.push(spec);
                    entry.insert(file_spec);
                }
                Entry::Occupied(entry) => {
                    let mut file_spec = entry.into_mut();
                    file_spec.push(spec);
                }
            }
        }

        Ok(files)
    }

    fn setup_module_path(&self, root_dir: &PathBuf, package: &Package) -> Result<PathBuf> {
        let package = self.package(package);

        let mut full_path = root_dir.to_owned();
        let mut iter = package.parts.iter().peekable();

        while let Some(part) = iter.next() {
            full_path = full_path.join(part);
        }

        if let Some(parent) = full_path.parent() {
            if !parent.is_dir() {
                fs::create_dir_all(parent)?;
            }
        }

        // path to final file
        full_path.set_extension(EXT);
        Ok(full_path)
    }

    fn write_files(&self, files: HashMap<&Package, FileSpec>) -> Result<()> {
        let root_dir = &self.options.parent.out_path;

        for (package, file_spec) in files {
            let full_path = self.setup_module_path(root_dir, package)?;

            debug!("+module: {}", full_path.display());

            let out = file_spec.format();
            let mut f = File::create(full_path)?;
            let bytes = out.into_bytes();

            f.write_all(&bytes)?;
            f.flush()?;
        }

        Ok(())
    }

    fn interface_decode_method(&self, interface: &InterfaceBody) -> Result<MethodSpec> {
        let mut decode = MethodSpec::with_static("decode");

        let data = stmt!["data"];

        decode.push_argument(&data);

        let mut body = Elements::new();

        let type_field = Variable::Literal("f_type".to_owned());

        body.push(stmt!["const ", &type_field, " = ", &data, "[", &self.type_var, "]"]);

        for (_, ref sub_type) in &interface.sub_types {
            for name in &sub_type.names {
                let type_name: Variable = Name::local(&sub_type.name).into();
                let cond = stmt![&type_field, " === ", string(&name.inner)];
                body.push(js![if cond, js![return type_name, ".decode(", &data, ")"]]);
            }
        }

        body.push(js![throw string("bad type")]);
        decode.push(body.join(ElementSpec::Spacing));

        Ok(decode)
    }
}

impl Backend for Processor {
    fn process(&self) -> Result<()> {
        let files = self.populate_files()?;
        self.write_files(files)
    }

    fn verify(&self) -> Result<Vec<Error>> {
        Ok(vec![])
    }
}
