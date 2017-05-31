use backend::*;
use backend::errors::*;
use backend::for_context::ForContext;
use backend::models as m;
use codeviz::python::*;
use naming::{self, FromNaming};
use options::Options;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fs::File;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

const TYPE: &str = "type";
const INIT_PY: &str = "__init__.py";
const EXT: &str = "py";
const PYTHON_CONTEXT: &str = "python";

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

#[derive(Clone)]
pub struct Field {
    pub modifier: m::Modifier,
    pub ty: m::Type,
    pub name: String,
    pub ident: String,
}

impl Field {
    pub fn new(modifier: m::Modifier, ty: m::Type, name: String, ident: String) -> Field {
        Field {
            modifier: modifier,
            ty: ty,
            name: name,
            ident: ident,
        }
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
            build_getters: true,
            build_constructor: true,
        }
    }
}

pub struct Processor {
    options: ProcessorOptions,
    env: Environment,
    package_prefix: Option<m::Package>,
    listeners: Box<Listeners>,
    to_lower_snake: Box<naming::Naming>,
    staticmethod: BuiltInName,
    classmethod: BuiltInName,
    dict: BuiltInName,
    enum_enum: ImportedName,
    enum_auto: ImportedName,
    type_var: Variable,
}

impl Processor {
    pub fn new(options: ProcessorOptions,
               env: Environment,
               package_prefix: Option<m::Package>,
               listeners: Box<Listeners>)
               -> Processor {
        Processor {
            options: options,
            env: env,
            package_prefix: package_prefix,
            listeners: listeners,
            to_lower_snake: naming::SnakeCase::new().to_lower_snake(),
            staticmethod: Name::built_in("staticmethod"),
            classmethod: Name::built_in("classmethod"),
            dict: Name::built_in("dict"),
            enum_enum: Name::imported("enum", "Enum"),
            enum_auto: Name::imported("enum", "auto"),
            type_var: Variable::String(TYPE.to_owned()),
        }
    }

    fn find_field<'a>(&self,
                      fields: &'a Vec<m::Token<Field>>,
                      name: &str)
                      -> Option<(usize, &'a Field)> {
        for (i, field) in fields.iter().enumerate() {
            if field.name == name {
                return Some((i, &field.inner));
            }
        }

        None
    }

    /// Build a function that raises an exception if the given value `stmt` is None.
    fn raise_if_none(&self, stmt: &Statement, field: &Field) -> Elements {
        let mut raise_if_none = Elements::new();
        let required_error = Variable::String(format!("{}: is a required field", field.name));

        raise_if_none.push(stmt!["if ", &stmt, " is None:"]);
        raise_if_none.push_nested(stmt!["raise Exception(", required_error, ")"]);

        raise_if_none
    }

    fn encode_method<E>(&self,
                        package: &m::Package,
                        fields: &Vec<m::Token<Field>>,
                        builder: &BuiltInName,
                        extra: E)
                        -> Result<MethodSpec>
        where E: FnOnce(&mut Elements) -> ()
    {
        let mut encode = MethodSpec::new("encode");
        encode.push_argument(stmt!["self"]);

        let mut encode_body = Elements::new();

        encode_body.push(stmt!["data = ", builder, "()"]);

        extra(&mut encode_body);

        for field in fields {
            let var_string = Variable::String(field.ident.to_owned());
            let field_stmt = stmt!["self.", &field.ident];
            let value_stmt = self.encode(package, &field.ty, &field_stmt)?;

            match field.modifier {
                m::Modifier::Optional => {
                    let mut check_if_none = Elements::new();

                    check_if_none.push(stmt!["if ", &field_stmt, " is not None:"]);

                    let stmt = stmt!["data[", var_string, "] = ", value_stmt];

                    check_if_none.push_nested(stmt);

                    encode_body.push(check_if_none);
                }
                _ => {
                    encode_body.push(self.raise_if_none(&field_stmt, field));

                    let stmt = stmt!["data[", var_string, "] = ", value_stmt];

                    encode_body.push(stmt);
                }
            }
        }

        encode_body.push(stmt!["return data"]);

        encode.push(encode_body.join(ElementSpec::Spacing));
        Ok(encode)
    }

    fn encode_tuple_method(&self,
                           package: &m::Package,
                           fields: &Vec<m::Token<Field>>)
                           -> Result<MethodSpec> {
        let mut values = Statement::new();

        let mut encode = MethodSpec::new("encode");
        encode.push_argument(stmt!["self"]);

        let mut encode_body = Elements::new();

        for field in fields {
            let stmt = stmt!["self.", &field.ident];
            encode_body.push(self.raise_if_none(&stmt, field));
            values.push(self.encode(package, &field.ty, stmt)?);
        }

        encode_body.push(stmt!["return (", values.join(", "), ")"]);
        encode.push(encode_body.join(ElementSpec::Spacing));
        Ok(encode)
    }

    fn encode_enum_method(&self, field: &Field) -> Result<MethodSpec> {
        let mut encode = MethodSpec::new("encode");
        encode.push_argument(stmt!["self"]);

        let mut encode_body = Elements::new();

        encode_body.push(stmt!["return self.", &field.ident]);
        encode.push(encode_body.join(ElementSpec::Spacing));
        Ok(encode)
    }

    fn decode_enum_method(&self, field: &Field) -> Result<MethodSpec> {
        let mut decode = MethodSpec::new("decode");

        let cls = stmt!["cls"];
        let data = stmt!["data"];

        decode.push_decorator(&self.classmethod);
        decode.push_argument(&cls);
        decode.push_argument(&data);

        let mut decode_body = Elements::new();

        let value = stmt!["value"];

        let mut check = Elements::new();
        check.push(stmt!["if ", &value, ".", &field.ident, " == ", data, ":"]);
        check.push_nested(stmt!["return ", &value]);

        let mut member_loop = Elements::new();

        member_loop.push(stmt!["for ", &value, " in ", &cls, ".__members__.values():"]);
        member_loop.push_nested(check);

        let mismatch = Variable::String("data does not match enum".to_owned());
        let raise = stmt!["raise Exception(", mismatch, ")"];

        decode_body.push(member_loop);
        decode_body.push(raise);

        decode.push(decode_body.join(ElementSpec::Spacing));
        Ok(decode)
    }

    fn optional_check(&self, var_name: &str, index: &Variable, stmt: &Statement) -> ElementSpec {
        let mut check = Elements::new();

        let mut none_check = Elements::new();
        none_check.push(stmt![var_name, " = data[", index, "]"]);

        let mut none_check_if = Elements::new();

        let assign_var = stmt![var_name, " = ", stmt];

        none_check_if.push(stmt!["if ", var_name, " is not None:"]);
        none_check_if.push_nested(assign_var);

        none_check.push(none_check_if);

        check.push(stmt!["if ", index, " in data:"]);
        check.push_nested(none_check.join(ElementSpec::Spacing));

        check.push(stmt!["else:"]);
        check.push_nested(stmt![var_name, " = None"]);

        check.into()
    }

    fn decode_method<F>(&self,
                        package: &m::Package,
                        fields: &Vec<m::Token<Field>>,
                        class: &ClassSpec,
                        variable_fn: F)
                        -> Result<MethodSpec>
        where F: Fn(usize, &Field) -> Variable
    {
        let mut decode = MethodSpec::new("decode");
        decode.push_decorator(&self.staticmethod);
        decode.push_argument(stmt!["data"]);

        let mut decode_body = Elements::new();

        let mut arguments = Statement::new();

        for (i, field) in fields.iter().enumerate() {
            let var_name = format!("f_{}", field.ident);
            let var = variable_fn(i, field);

            let stmt = match field.modifier {
                m::Modifier::Optional => {
                    let var_stmt = self.decode(&field.pos, package, &field.ty, &var_name)?;
                    self.optional_check(&var_name, &var, &var_stmt)
                }
                _ => {
                    let var_stmt = stmt!["data[", &var, "]"];
                    let var_stmt = self.decode(&field.pos, package, &field.ty, var_stmt)?;
                    stmt![&var_name, " = ", &var_stmt].into()
                }
            };

            decode_body.push(stmt);
            arguments.push(var_name);
        }

        let arguments = arguments.join(", ");
        decode_body.push(stmt!["return ", &class.name, "(", arguments, ")"]);

        decode.push(decode_body.join(ElementSpec::Spacing));

        Ok(decode)
    }

    fn is_native(&self, ty: &m::Type) -> bool {
        match *ty {
            m::Type::Signed(_) |
            m::Type::Unsigned(_) => true,
            m::Type::Float | m::Type::Double => true,
            m::Type::String => true,
            m::Type::Any => true,
            m::Type::Boolean => true,
            m::Type::Array(ref inner) => self.is_native(inner),
            _ => false,
        }
    }

    fn ident(&self, name: &str) -> String {
        if let Some(ref id_converter) = self.options.parent.id_converter {
            id_converter.convert(name)
        } else {
            name.to_owned()
        }
    }

    fn used_name(&self,
                 pos: &m::Pos,
                 package: &m::Package,
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

    fn encode<S>(&self, package: &m::Package, ty: &m::Type, value_stmt: S) -> Result<Statement>
        where S: Into<Statement>
    {
        let value_stmt = value_stmt.into();

        // TODO: do not skip conversion if strict type checking is enabled
        if self.is_native(ty) {
            return Ok(value_stmt);
        }

        let value_stmt = match *ty {
            m::Type::Signed(_) |
            m::Type::Unsigned(_) => value_stmt,
            m::Type::Float | m::Type::Double => value_stmt,
            m::Type::String => value_stmt,
            m::Type::Any => value_stmt,
            m::Type::Boolean => value_stmt,
            m::Type::Custom(ref _used, ref _custom) => stmt![value_stmt, ".encode()"],
            m::Type::Array(ref inner) => {
                let v = stmt!["v"];
                let inner = self.encode(package, inner, v)?;
                stmt!["map(lambda v: ", inner, ", ", value_stmt, ")"]
            }
            _ => value_stmt,
        };

        Ok(value_stmt)
    }

    fn decode<S>(&self,
                 pos: &m::Pos,
                 package: &m::Package,
                 ty: &m::Type,
                 value_stmt: S)
                 -> Result<Statement>
        where S: Into<Statement>
    {
        let value_stmt = value_stmt.into();

        // TODO: do not skip conversion if strict type checking is enabled
        if self.is_native(ty) {
            return Ok(value_stmt);
        }

        let value_stmt = match *ty {
            m::Type::Signed(_) |
            m::Type::Unsigned(_) => value_stmt,
            m::Type::Float | m::Type::Double => value_stmt,
            m::Type::String => value_stmt,
            m::Type::Any => value_stmt,
            m::Type::Boolean => value_stmt,
            m::Type::Custom(ref used, ref custom) => {
                let name = self.used_name(pos, package, used, custom)?;
                stmt![name, ".decode(", value_stmt, ")"]
            }
            m::Type::Array(ref inner) => {
                let inner = self.decode(pos, package, inner, stmt!["v"])?;
                stmt!["map(lambda v: ", inner, ", ", value_stmt, ")"]
            }
            _ => value_stmt,
        };

        Ok(value_stmt)
    }


    /// Build the java package of a given package.
    ///
    /// This includes the prefixed configured in `self.options`, if specified.
    fn package(&self, package: &m::Package) -> m::Package {
        self.package_prefix
            .clone()
            .map(|prefix| prefix.join(package))
            .unwrap_or_else(|| package.clone())
    }

    fn build_constructor(&self, fields: &Vec<m::Token<Field>>) -> MethodSpec {
        let mut constructor = MethodSpec::new("__init__");
        constructor.push_argument(stmt!["self"]);

        for field in fields {
            constructor.push_argument(stmt![&field.ident]);
            constructor.push(stmt!["self.", &field.ident, " = ", &field.ident]);
        }

        constructor
    }

    fn process_tuple(&self, package: &m::Package, body: &m::TupleBody) -> Result<ClassSpec> {
        let mut class = ClassSpec::new(&body.name);
        let mut fields: Vec<m::Token<Field>> = Vec::new();

        for field in &body.fields {
            let ident = self.ident(&field.name);

            fields.push(field.clone()
                .map_inner(|f| Field::new(m::Modifier::Required, f.ty, f.name, ident)));
        }

        class.push(self.build_constructor(&fields));

        // TODO: make configurable
        if false {
            for getter in self.build_getters(&fields)? {
                class.push(&getter);
            }
        }

        for code in body.codes.for_context(PYTHON_CONTEXT) {
            class.push(code.inner.lines);
        }

        self.tuple_added(package, &fields, &mut class)?;
        Ok(class)
    }

    fn literal_value(&self, pos: &m::Pos, value: &m::Value, ty: &m::Type) -> Result<Variable> {
        match *ty {
            m::Type::Double |
            m::Type::Float |
            m::Type::Signed(_) |
            m::Type::Unsigned(_) |
            m::Type::Boolean => {
                if let m::Value::Boolean(ref boolean) = *value {
                    return Ok(Variable::Literal(boolean.to_string()));
                }

                if let m::Value::Number(ref number) = *value {
                    return Ok(Variable::Literal(number.to_string()));
                }
            }
            m::Type::String => {
                if let m::Value::String(ref string) = *value {
                    return Ok(Variable::String(string.to_owned()));
                }
            }
            _ => {}
        }

        Err(Error::pos(format!("{} cannot be applied to expected type {}", value, ty),
                       pos.clone()))
    }

    fn process_enum(&self, _package: &m::Package, body: &m::EnumBody) -> Result<ClassSpec> {
        let mut class = ClassSpec::new(&body.name);
        let mut fields: Vec<m::Token<Field>> = Vec::new();

        for field in &body.fields {
            let ident = self.ident(&field.name);

            // reserved fields
            let ident = match ident.as_str() {
                "name" => "_name".to_owned(),
                "value" => "_value".to_owned(),
                i => i.to_owned(),
            };

            fields.push(field.clone()
                .map_inner(|f| Field::new(m::Modifier::Required, f.ty, f.name, ident)));
        }

        class.extends(&self.enum_enum);

        let mut values = Elements::new();

        for value in &body.values {
            let arguments = if !value.arguments.is_empty() {
                let mut value_arguments = Statement::new();

                for (value, field) in value.arguments.iter().zip(fields.iter()) {
                    value_arguments.push(self.literal_value(&value.pos, value, &field.ty)?);
                }

                stmt!["(", value_arguments.join(", "), ")"]
            } else {
                stmt![&self.enum_auto, "()"]
            };

            values.push(stmt![&value.name, " = ", arguments]);
        }

        class.push(values);

        class.push(self.build_constructor(&fields));

        // TODO: make configurable
        if false {
            for getter in self.build_getters(&fields)? {
                class.push(&getter);
            }
        }

        for code in body.codes.for_context(PYTHON_CONTEXT) {
            class.push(code.inner.lines);
        }

        let serialized_as = &body.serialized_as;
        self.enum_added(&fields, serialized_as, &mut class)?;
        Ok(class)
    }

    fn build_getters(&self, fields: &Vec<m::Token<Field>>) -> Result<Vec<MethodSpec>> {
        let mut result = Vec::new();

        for field in fields {
            let name = self.to_lower_snake.convert(&field.ident);
            let getter_name = format!("get_{}", name);
            let mut method_spec = MethodSpec::new(&getter_name);
            method_spec.push_argument(stmt!["self"]);
            method_spec.push(stmt!["return self.", name]);
            result.push(method_spec);
        }

        Ok(result)
    }

    fn process_type(&self, package: &m::Package, body: &m::TypeBody) -> Result<ClassSpec> {
        let mut class = ClassSpec::new(&body.name);
        let mut fields = Vec::new();

        for field in &body.fields {
            let ident = self.ident(&field.name);

            fields.push(field.clone().map_inner(|f| Field::new(f.modifier, f.ty, f.name, ident)));
        }

        let constructor = self.build_constructor(&fields);
        class.push(&constructor);

        // TODO: make configurable
        if false {
            for getter in self.build_getters(&fields)? {
                class.push(getter);
            }
        }

        let decode = self.decode_method(package,
                           &fields,
                           &class,
                           |_, field| Variable::String(field.ident.to_owned()))?;

        class.push(decode);

        let encode = self.encode_method(package, &fields, &self.dict, |_| {})?;

        class.push(encode);

        for code in body.codes.for_context(PYTHON_CONTEXT) {
            class.push(code.inner.lines);
        }

        Ok(class)
    }

    fn process_interface(&self,
                         package: &m::Package,
                         body: &m::InterfaceBody)
                         -> Result<Vec<ClassSpec>> {
        let mut classes = Vec::new();

        let mut interface_spec = ClassSpec::new(&body.name);

        interface_spec.push(self.interface_decode_method(body)?);

        let mut interface_fields = Vec::new();

        for field in &body.fields {
            let ident = self.ident(&field.name);

            interface_fields.push(field.clone().map_inner(|f| {
                    Field::new(f.modifier, f.ty, f.name, ident)
                }));
        }

        for code in body.codes.for_context(PYTHON_CONTEXT) {
            interface_spec.push(code.inner.lines);
        }

        classes.push(interface_spec);

        for (_, ref sub_type) in &body.sub_types {
            let mut class = ClassSpec::new(&sub_type.name);
            class.extends(Name::local(&body.name));

            class.push(stmt!["TYPE = ", Variable::String(sub_type.name())]);

            let mut fields = interface_fields.clone();

            for field in &sub_type.fields {
                let ident = self.ident(&field.name);

                fields.push(field.clone().map_inner(|f| {
                    Field::new(f.modifier, f.ty, f.name, ident)
                }));
            }

            let constructor = self.build_constructor(&fields);
            class.push(&constructor);

            // TODO: make configurable
            if false {
                for getter in self.build_getters(&fields)? {
                    class.push(&getter);
                }
            }

            let decode = self.decode_method(package,
                               &fields,
                               &class,
                               |_, field| Variable::String(field.ident.to_owned()))?;

            class.push(decode);

            let type_stmt =
                stmt!["data[", &self.type_var, "] = ", Variable::String(sub_type.name())];

            let encode = self.encode_method(package, &fields, &self.dict, move |elements| {
                    elements.push(type_stmt);
                })?;

            class.push(encode);

            for code in sub_type.codes.for_context(PYTHON_CONTEXT) {
                class.push(code.inner.lines);
            }

            classes.push(class);
        }

        Ok(classes)
    }

    fn populate_files(&self) -> Result<HashMap<&m::Package, FileSpec>> {
        let mut files = HashMap::new();

        // Process all types discovered so far.
        for (&(ref package, _), decl) in &self.env.types {
            let class_specs: Vec<ClassSpec> = match decl.inner {
                m::Decl::Interface(ref body) => self.process_interface(package, body)?,
                m::Decl::Type(ref body) => vec![self.process_type(package, body)?],
                m::Decl::Tuple(ref body) => vec![self.process_tuple(package, body)?],
                m::Decl::Enum(ref body) => vec![self.process_enum(package, body)?],
            };

            match files.entry(package) {
                Entry::Vacant(entry) => {
                    let mut file_spec = FileSpec::new();

                    for class_spec in class_specs {
                        file_spec.push(class_spec);
                    }

                    entry.insert(file_spec);
                }
                Entry::Occupied(entry) => {
                    let mut file_spec = entry.into_mut();

                    for class_spec in class_specs {
                        file_spec.push(class_spec);
                    }
                }
            }
        }

        Ok(files)
    }

    fn setup_module_path(&self, root_dir: &PathBuf, package: &m::Package) -> Result<PathBuf> {
        let package = self.package(package);

        let mut full_path = root_dir.to_owned();
        let mut iter = package.parts.iter().peekable();

        while let Some(part) = iter.next() {
            full_path = full_path.join(part);

            if iter.peek().is_none() {
                continue;
            }

            let init_path = full_path.join(INIT_PY);

            if !init_path.is_file() {
                if !full_path.is_dir() {
                    debug!("+dir: {}", full_path.display());
                    fs::create_dir_all(&full_path)?;
                }

                debug!("+init: {}", init_path.display());
                File::create(init_path)?;
            }
        }

        // path to final file
        full_path.set_extension(EXT);
        Ok(full_path)
    }

    fn write_files(&self, files: HashMap<&m::Package, FileSpec>) -> Result<()> {
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

    fn tuple_added(&self,
                   package: &m::Package,
                   fields: &Vec<m::Token<Field>>,
                   class: &mut ClassSpec)
                   -> Result<()> {

        let decode = self.decode_method(package,
                           fields,
                           class,
                           |i, _| Variable::Literal(i.to_string()))?;

        let encode = self.encode_tuple_method(package, fields)?;

        class.push(decode);
        class.push(encode);
        Ok(())
    }

    fn enum_added(&self,
                  fields: &Vec<m::Token<Field>>,
                  serialized_as: &Option<m::Token<String>>,
                  class: &mut ClassSpec)
                  -> Result<()> {
        if let Some(ref s) = *serialized_as {
            if let Some((_, ref field)) = self.find_field(fields, &s.inner) {
                class.push(self.encode_enum_method(field)?);
                class.push(self.decode_enum_method(field)?);
            } else {
                return Err(Error::pos(format!("no field named: {}", s.inner), s.pos.clone()));
            }
        }

        Ok(())
    }

    fn interface_decode_method(&self, interface: &m::InterfaceBody) -> Result<MethodSpec> {
        let mut decode = MethodSpec::new("decode");
        decode.push_decorator(&self.staticmethod);
        decode.push_argument(stmt!["data"]);

        let mut decode_body = Elements::new();

        let type_field = Variable::Literal("f_type".to_owned());

        decode_body.push(stmt![&type_field, " = data[", &self.type_var, "]"]);

        for (_, ref sub_type) in &interface.sub_types {
            for name in &sub_type.names {
                let type_name: Variable = Name::local(&sub_type.name).into();

                let mut check = Elements::new();

                check.push(stmt!["if ",
                                 &type_field,
                                 " == ",
                                 Variable::String(name.inner.to_owned()),
                                 ":"]);
                check.push_nested(stmt!["return ", type_name, ".decode(data)"]);

                decode_body.push(check);
            }
        }

        decode_body.push(stmt!["raise Exception(",
                               Variable::String("bad type".to_owned()),
                               " + ",
                               &type_field,
                               ")"]);

        decode.push(decode_body.join(ElementSpec::Spacing));

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

impl ::std::fmt::Display for m::Type {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        match *self {
            m::Type::Signed(_) |
            m::Type::Unsigned(_) => write!(f, "int"),
            m::Type::Float | m::Type::Double => write!(f, "float"),
            m::Type::String => write!(f, "str"),
            m::Type::Custom(ref used, ref custom) => {
                if let Some(ref used) = *used {
                    write!(f, "{}.{}", used, custom.join("."))
                } else {
                    write!(f, "{}", custom.join("."))
                }
            }
            m::Type::Array(_) => write!(f, "array"),
            m::Type::Boolean => write!(f, "bool"),
            _ => write!(f, "<unknown>"),
        }
    }
}
