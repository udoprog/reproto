use backend::*;
use backend::collecting::Collecting;
use backend::errors::*;
use backend::for_context::ForContext;
use backend::package_processor::PackageProcessor;
use backend::value_builder::*;
use backend::variables::Variables;
use codeviz::python::*;
use core::*;
use naming::{self, FromNaming};
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use super::converter::Converter;
use super::dynamic_converter::DynamicConverter;
use super::dynamic_decode::DynamicDecode;
use super::dynamic_encode::DynamicEncode;
use super::match_decode::MatchDecode;

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
    pub modifier: RpModifier,
    pub ty: RpType,
    pub name: String,
    pub ident: String,
}

impl Field {
    pub fn new(modifier: RpModifier, ty: RpType, name: String, ident: String) -> Field {
        Field {
            modifier: modifier,
            ty: ty,
            name: name,
            ident: ident,
        }
    }
}

pub struct ProcessorOptions {
    pub build_getters: bool,
    pub build_constructor: bool,
}

impl ProcessorOptions {
    pub fn new() -> ProcessorOptions {
        ProcessorOptions {
            build_getters: true,
            build_constructor: true,
        }
    }
}

pub struct Processor {
    env: Environment,
    out_path: PathBuf,
    id_converter: Option<Box<naming::Naming>>,
    package_prefix: Option<RpPackage>,
    listeners: Box<Listeners>,
    to_lower_snake: Box<naming::Naming>,
    staticmethod: BuiltInName,
    classmethod: BuiltInName,
    isinstance: BuiltInName,
    dict: BuiltInName,
    list: BuiltInName,
    basestring: BuiltInName,
    boolean: BuiltInName,
    number: ImportedName,
    enum_enum: ImportedName,
    type_var: Variable,
}

impl Processor {
    pub fn new(_options: ProcessorOptions,
               env: Environment,
               out_path: PathBuf,
               id_converter: Option<Box<naming::Naming>>,
               package_prefix: Option<RpPackage>,
               listeners: Box<Listeners>)
               -> Processor {
        Processor {
            env: env,
            out_path: out_path,
            id_converter: id_converter,
            package_prefix: package_prefix,
            listeners: listeners,
            to_lower_snake: naming::SnakeCase::new().to_lower_snake(),
            staticmethod: Name::built_in("staticmethod"),
            classmethod: Name::built_in("classmethod"),
            isinstance: Name::built_in("isinstance"),
            dict: Name::built_in("dict"),
            list: Name::built_in("list"),
            basestring: Name::built_in("basestring"),
            boolean: Name::built_in("bool"),
            number: Name::imported("numbers", "Number"),
            enum_enum: Name::imported("enum", "Enum"),
            type_var: Variable::String(TYPE.to_owned()),
        }
    }

    fn find_field<'a>(&self,
                      fields: &'a Vec<RpLoc<Field>>,
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
                        type_id: &RpTypeId,
                        fields: &Vec<RpLoc<Field>>,
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
            let value_stmt = self.encode(type_id, &field.pos, &field.ty, &field_stmt)?;

            match field.modifier {
                RpModifier::Optional => {
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

        encode.push(encode_body.join(Spacing));
        Ok(encode)
    }

    fn encode_tuple_method(&self,
                           type_id: &RpTypeId,
                           fields: &Vec<RpLoc<Field>>)
                           -> Result<MethodSpec> {
        let mut values = Statement::new();

        let mut encode = MethodSpec::new("encode");
        encode.push_argument(stmt!["self"]);

        let mut encode_body = Elements::new();

        for field in fields {
            let stmt = stmt!["self.", &field.ident];
            encode_body.push(self.raise_if_none(&stmt, field));
            values.push(self.encode(type_id, &field.pos, &field.ty, &stmt)?);
        }

        encode_body.push(stmt!["return (", values.join(", "), ")"]);
        encode.push(encode_body.join(Spacing));
        Ok(encode)
    }

    fn encode_enum_method(&self, field: &Field) -> Result<MethodSpec> {
        let mut encode = MethodSpec::new("encode");
        encode.push_argument(stmt!["self"]);

        let mut encode_body = Elements::new();

        encode_body.push(stmt!["return self.", &field.ident]);
        encode.push(encode_body.join(Spacing));
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

        decode.push(decode_body.join(Spacing));
        Ok(decode)
    }

    fn optional_check(&self, var_name: &Statement, index: &Variable, stmt: &Statement) -> Element {
        let mut check = Elements::new();

        let mut none_check = Elements::new();
        none_check.push(stmt![var_name, " = data[", index, "]"]);

        let mut none_check_if = Elements::new();

        let assign_var = stmt![var_name, " = ", stmt];

        none_check_if.push(stmt!["if ", var_name, " is not None:"]);
        none_check_if.push_nested(assign_var);

        none_check.push(none_check_if);

        check.push(stmt!["if ", index, " in data:"]);
        check.push_nested(none_check.join(Spacing));

        check.push(stmt!["else:"]);
        check.push_nested(stmt![var_name, " = None"]);

        check.into()
    }

    fn decode_method<F>(&self,
                        type_id: &RpTypeId,
                        pos: &RpPos,
                        match_decl: &RpMatchDecl,
                        fields: &Vec<RpLoc<Field>>,
                        variable_fn: F)
                        -> Result<MethodSpec>
        where F: Fn(usize, &Field) -> Variable
    {
        let data = stmt!["data"];

        let mut decode = MethodSpec::new("decode");
        decode.push_decorator(&self.staticmethod);
        decode.push_argument(&data);

        let mut decode_body = Elements::new();

        if let Some(by_value) = self.decode_by_value(type_id, match_decl, &data)? {
            decode_body.push(by_value.join(Spacing));
        }

        if let Some(by_type) = self.decode_by_type(type_id, match_decl, &data)? {
            decode_body.push(by_type.join(Spacing));
        }

        let mut arguments = Statement::new();

        for (i, field) in fields.iter().enumerate() {
            let var_name = format!("f_{}", field.ident);
            let var = variable_fn(i, field);

            let stmt = match field.modifier {
                RpModifier::Optional => {
                    let var_name = var_name.clone().into();
                    let var_stmt = self.decode(type_id, &field.pos, &field.ty, &var_name)?;
                    self.optional_check(&var_name, &var, &var_stmt)
                }
                _ => {
                    let var_stmt = stmt!["data[", &var, "]"];
                    let var_stmt = self.decode(type_id, &field.pos, &field.ty, &var_stmt.into())?;
                    stmt![&var_name, " = ", &var_stmt].into()
                }
            };

            decode_body.push(stmt);
            arguments.push(var_name);
        }

        let arguments = arguments.join(", ");
        let name = self.convert_type(pos, type_id)?;
        decode_body.push(stmt!["return ", name, "(", arguments, ")"]);

        decode.push(decode_body.join(Spacing));

        Ok(decode)
    }

    fn ident(&self, name: &str) -> String {
        if let Some(ref id_converter) = self.id_converter {
            id_converter.convert(name)
        } else {
            name.to_owned()
        }
    }

    /// Build the java package of a given package.
    ///
    /// This includes the prefixed configured in `self.options`, if specified.
    fn package(&self, package: &RpPackage) -> RpPackage {
        self.package_prefix
            .clone()
            .map(|prefix| prefix.join(package))
            .unwrap_or_else(|| package.clone())
    }

    fn build_constructor(&self, fields: &Vec<RpLoc<Field>>) -> MethodSpec {
        let mut constructor = MethodSpec::new("__init__");
        constructor.push_argument(stmt!["self"]);

        for field in fields {
            constructor.push_argument(stmt![&field.ident]);
            constructor.push(stmt!["self.", &field.ident, " = ", &field.ident]);
        }

        constructor
    }

    fn build_getters(&self, fields: &Vec<RpLoc<Field>>) -> Result<Vec<MethodSpec>> {
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

    fn populate_files(&self) -> Result<HashMap<&RpPackage, FileSpec>> {
        let mut enums = Vec::new();

        let mut files = self.do_populate_files(|type_id, decl| {
                if let RpDecl::Enum(ref body) = decl.inner {
                    enums.push((type_id, body));
                }

                Ok(())
            })?;

        /// process static initialization of enums at bottom of file
        for (type_id, body) in enums {
            if let Some(ref mut file_spec) = files.get_mut(&type_id.package) {
                file_spec.push(self.enum_variants(type_id, body)?);
            } else {
                return Err(format!("no such package: {}", &type_id.package).into());
            }
        }

        Ok(files)
    }

    fn setup_module_path(&self, root_dir: &PathBuf, package: &RpPackage) -> Result<PathBuf> {
        let package = self.package(package);

        let mut full_path = root_dir.to_owned();
        let mut iter = package.parts.iter().peekable();

        while let Some(part) = iter.next() {
            full_path = full_path.join(part);

            if iter.peek().is_none() {
                continue;
            }

            if !full_path.is_dir() {
                debug!("+dir: {}", full_path.display());
                fs::create_dir_all(&full_path)?;
            }

            let init_path = full_path.join(INIT_PY);

            if !init_path.is_file() {
                debug!("+init: {}", init_path.display());
                File::create(init_path)?;
            }
        }

        if let Some(parent) = full_path.parent() {
            if !parent.is_dir() {
                debug!("+dir: {}", parent.display());
                fs::create_dir_all(&parent)?;
            }
        }

        // path to final file
        full_path.set_extension(EXT);
        Ok(full_path)
    }

    fn write_files(&self, files: HashMap<&RpPackage, FileSpec>) -> Result<()> {
        let root_dir = &self.out_path;

        for (package, file_spec) in files {
            let full_path = self.setup_module_path(root_dir, package)?;

            debug!("+module: {}", full_path.display());

            let mut out = String::new();
            file_spec.format(&mut out)?;

            let mut f = File::create(full_path)?;
            f.write_all(&out.into_bytes())?;
            f.flush()?;
        }

        Ok(())
    }

    fn convert_type_id<F>(&self, pos: &RpPos, type_id: &RpTypeId, path_syntax: F) -> Result<Name>
        where F: Fn(&Vec<String>) -> String
    {
        let package = &type_id.package;
        let name = &type_id.name;

        if let Some(ref used) = name.prefix {
            let package = self.env
                .lookup_used(package, used)
                .map_err(|e| Error::pos(e.description().to_owned(), pos.clone()))?;

            let package = self.package(package);
            let package = package.parts.join(".");
            return Ok(Name::imported_alias(&package, &path_syntax(&name.parts), used).into());
        }

        // no nested types in python
        Ok(Name::local(&path_syntax(&name.parts)).into())
    }

    fn enum_variants(&self, type_id: &RpTypeId, body: &RpEnumBody) -> Result<Statement> {
        let mut arguments = Statement::new();

        let variables = Variables::new();

        for variant in &body.variants {
            let name = Variable::String((*variant.name).to_owned());

            let mut enum_arguments = Statement::new();

            enum_arguments.push(name);

            if !variant.arguments.is_empty() {
                let mut value_arguments = Statement::new();

                for (value, field) in variant.arguments.iter().zip(body.fields.iter()) {
                    let env = ValueBuilderEnv {
                        value: &value,
                        package: &type_id.package,
                        ty: Some(&field.ty),
                        variables: &variables,
                    };

                    value_arguments.push(self.value(&env)?);
                }

                enum_arguments.push(stmt!["(", value_arguments.join(", "), ")"]);
            } else {
                enum_arguments.push(variant.ordinal.to_string());
            }

            arguments.push(stmt!["(", enum_arguments.join(", "), ")"]);
        }

        let class_name = Variable::String(body.name.to_owned());

        Ok(stmt![&body.name,
                 " = ",
                 &self.enum_enum,
                 "(",
                 class_name,
                 ", [",
                 arguments.join(", "),
                 "], type=",
                 &body.name,
                 ")"])
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

impl Collecting for FileSpec {
    type Processor = Processor;

    fn new() -> Self {
        FileSpec::new()
    }

    fn into_bytes(self, _: &Self::Processor) -> Result<Vec<u8>> {
        let mut out = String::new();
        self.format(&mut out)?;
        Ok(out.into_bytes())
    }
}

impl PackageProcessor for Processor {
    type Out = FileSpec;

    fn ext(&self) -> &str {
        EXT
    }

    fn env(&self) -> &Environment {
        &self.env
    }

    fn package_prefix(&self) -> &Option<RpPackage> {
        &self.package_prefix
    }

    fn out_path(&self) -> &Path {
        &self.out_path
    }

    fn default_process(&self, _out: &mut Self::Out, type_id: &RpTypeId, _: &RpPos) -> Result<()> {
        Err(format!("not supported: {:?}", type_id).into())
    }

    fn process_tuple(&self,
                     out: &mut Self::Out,
                     type_id: &RpTypeId,
                     pos: &RpPos,
                     body: Rc<RpTupleBody>)
                     -> Result<()> {
        let mut class = ClassSpec::new(&body.name);
        let mut fields: Vec<RpLoc<Field>> = Vec::new();

        for field in &body.fields {
            let ident = self.ident(&field.name);

            fields.push(field.clone()
                .map_inner(|f| Field::new(RpModifier::Required, f.ty, f.name, ident)));
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

        let decode = self.decode_method(type_id,
                           pos,
                           &body.match_decl,
                           &fields,
                           |i, _| Variable::Literal(i.to_string()))?;
        class.push(decode);

        let encode = self.encode_tuple_method(&type_id, &fields)?;
        class.push(encode);

        out.push(class);
        Ok(())
    }

    fn process_enum(&self,
                    out: &mut Self::Out,
                    _: &RpTypeId,
                    _: &RpPos,
                    body: Rc<RpEnumBody>)
                    -> Result<()> {
        let mut class = ClassSpec::new(&body.name);
        let mut fields: Vec<RpLoc<Field>> = Vec::new();

        for field in &body.fields {
            let ident = self.ident(&field.name);

            // reserved fields
            let ident = match ident.as_str() {
                "name" => "_name".to_owned(),
                "value" => "_value".to_owned(),
                i => i.to_owned(),
            };

            fields.push(field.clone()
                .map_inner(|f| Field::new(RpModifier::Required, f.ty, f.name, ident)));
        }

        if !fields.is_empty() {
            class.push(self.build_constructor(&fields));
        }

        // TODO: make configurable
        if false {
            for getter in self.build_getters(&fields)? {
                class.push(&getter);
            }
        }

        for code in body.codes.for_context(PYTHON_CONTEXT) {
            class.push(code.inner.lines);
        }

        if let Some(ref s) = body.serialized_as {
            if let Some((_, ref field)) = self.find_field(&fields, &s.inner) {
                class.push(self.encode_enum_method(field)?);
                class.push(self.decode_enum_method(field)?);
            } else {
                return Err(Error::pos(format!("no field named: {}", s.inner), s.pos.clone()));
            }
        }

        out.push(class);
        Ok(())
    }

    fn process_type(&self,
                    out: &mut Self::Out,
                    type_id: &RpTypeId,
                    pos: &RpPos,
                    body: Rc<RpTypeBody>)
                    -> Result<()> {
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

        let decode = self.decode_method(type_id,
                           pos,
                           &body.match_decl,
                           &fields,
                           |_, field| Variable::String(field.ident.to_owned()))?;

        class.push(decode);

        let encode = self.encode_method(type_id, &fields, &self.dict, |_| {})?;

        class.push(encode);

        for code in body.codes.for_context(PYTHON_CONTEXT) {
            class.push(code.inner.lines);
        }

        out.push(class);
        Ok(())
    }

    fn process_interface(&self,
                         out: &mut Self::Out,
                         type_id: &RpTypeId,
                         _: &RpPos,
                         body: Rc<RpInterfaceBody>)
                         -> Result<()> {
        let mut interface_spec = ClassSpec::new(&body.name);

        interface_spec.push(self.interface_decode_method(type_id, &body)?);

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

        out.push(interface_spec);

        for (_, ref sub_type) in &body.sub_types {
            let mut class = ClassSpec::new(&format!("{}_{}", &body.name, &sub_type.name));
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

            let decode = self.decode_method(type_id,
                               &sub_type.pos,
                               &sub_type.match_decl,
                               &fields,
                               |_, field| Variable::String(field.ident.to_owned()))?;

            class.push(decode);

            let type_stmt =
                stmt!["data[", &self.type_var, "] = ", Variable::String(sub_type.name())];

            let encode = self.encode_method(type_id, &fields, &self.dict, move |elements| {
                    elements.push(type_stmt);
                })?;

            class.push(encode);

            for code in sub_type.codes.for_context(PYTHON_CONTEXT) {
                class.push(code.inner.lines);
            }

            out.push(class);
        }

        Ok(())
    }
}

impl Converter for Processor {
    type Type = Name;
    type Stmt = Statement;
    type Elements = Elements;
    type Variable = Variable;

    fn new_var(&self, name: &str) -> Self::Stmt {
        stmt![name]
    }

    fn convert_type(&self, pos: &RpPos, type_id: &RpTypeId) -> Result<Name> {
        self.convert_type_id(pos, type_id, |v| v.join("_"))
    }

    fn convert_constant(&self, pos: &RpPos, type_id: &RpTypeId) -> Result<Name> {
        self.convert_type_id(pos, type_id, |v| v.join("."))
    }
}

/// Build values in python.
impl ValueBuilder for Processor {
    fn env(&self) -> &Environment {
        &self.env
    }

    fn identifier(&self, identifier: &str) -> Result<Self::Stmt> {
        Ok(stmt![identifier])
    }

    fn optional_empty(&self) -> Result<Self::Stmt> {
        Ok(stmt!["None"])
    }

    fn constant(&self, ty: Self::Type) -> Result<Self::Stmt> {
        return Ok(stmt![ty]);
    }

    fn instance(&self, ty: Self::Type, arguments: Vec<Self::Stmt>) -> Result<Self::Stmt> {
        let mut stmt = Statement::new();

        for a in arguments {
            stmt.push(a);
        }

        Ok(stmt![&ty, "(", stmt.join(", "), ")"])
    }

    fn number(&self, number: &RpNumber) -> Result<Self::Stmt> {
        Ok(stmt![number.to_string()])
    }

    fn boolean(&self, boolean: &bool) -> Result<Self::Stmt> {
        Ok(stmt![boolean.to_string()])
    }

    fn string(&self, string: &str) -> Result<Self::Stmt> {
        Ok(Variable::String(string.to_owned()).into())
    }

    fn array(&self, values: Vec<Self::Stmt>) -> Result<Self::Stmt> {
        let mut arguments = Statement::new();

        for v in values {
            arguments.push(v);
        }

        Ok(stmt!["[", arguments.join(", "), "]"])
    }
}

impl DynamicConverter for Processor {
    fn is_native(&self, ty: &RpType) -> bool {
        match *ty {
            RpType::Signed(_) |
            RpType::Unsigned(_) => true,
            RpType::Float | RpType::Double => true,
            RpType::String => true,
            RpType::Any => true,
            RpType::Boolean => true,
            RpType::Array(ref inner) => self.is_native(inner),
            RpType::Map(ref key, ref value) => self.is_native(key) && self.is_native(value),
            _ => false,
        }
    }

    fn map_key_var(&self) -> Statement {
        stmt!["t[0]"]
    }

    fn map_value_var(&self) -> Statement {
        stmt!["t[1]"]
    }

    fn array_inner_var(&self) -> Statement {
        stmt!["v"]
    }
}

impl DynamicDecode for Processor {
    type Method = MethodSpec;

    fn name_decode(&self, input: &Statement, name: Self::Type) -> Self::Stmt {
        stmt![name, ".decode(", input, ")"]
    }

    fn array_decode(&self, input: &Statement, inner: Statement) -> Self::Stmt {
        stmt!["map(lambda v: ", inner, ", ", input, ")"]
    }

    fn map_decode(&self, input: &Statement, key: Statement, value: Statement) -> Self::Stmt {
        let body = stmt!["(", &key, ", ", &value, ")"];
        stmt![&self.dict, "(map(lambda t: ", &body, ", ", input, ".items()))"]
    }

    fn assign_type_var(&self, data: &Self::Stmt, type_var: &Self::Stmt) -> Self::Stmt {
        stmt![type_var, " = ", data, "[", &self.type_var, "]"]
    }

    fn check_type_var(&self,
                      _data: &Self::Stmt,
                      type_var: &Self::Stmt,
                      name: &RpLoc<String>,
                      type_name: &Self::Type)
                      -> Self::Elements {
        let mut check = Elements::new();
        check.push(stmt!["if ", type_var, " == ", Variable::String(name.inner.to_owned()), ":"]);
        check.push_nested(stmt!["return ", type_name, ".decode(data)"]);
        check
    }

    fn raise_bad_type(&self, type_var: &Self::Stmt) -> Self::Stmt {
        stmt!["raise Exception(", Variable::String("bad type".to_owned()), " + ", type_var, ")"]
    }

    fn new_decode_method(&self, data: &Self::Stmt, body: Self::Elements) -> Self::Method {
        let mut decode = MethodSpec::new("decode");
        decode.push_decorator(&self.staticmethod);
        decode.push_argument(&data);
        decode.push(body);
        decode
    }
}

impl DynamicEncode for Processor {
    fn name_encode(&self, input: &Statement, _: Self::Type) -> Self::Stmt {
        stmt![input, ".encode()"]
    }

    fn array_encode(&self, input: &Statement, inner: Statement) -> Self::Stmt {
        stmt!["map(lambda v: ", inner, ", ", input, ")"]
    }

    fn map_encode(&self, input: &Statement, key: Statement, value: Statement) -> Self::Stmt {
        let body = stmt!["(", &key, ", ", &value, ")"];
        stmt![&self.dict, "(", input, ".items().map(lambda t: ", &body, "))"]
    }
}

impl MatchDecode for Processor {
    fn match_value(&self,
                   data: &Statement,
                   _value: &RpValue,
                   value_stmt: Statement,
                   _result: &RpValue,
                   result_stmt: Statement)
                   -> Result<Elements> {
        let mut value_body = Elements::new();
        value_body.push(stmt!["if ", &data, " == ", value_stmt, ":"]);
        value_body.push_nested(stmt!["return ", result_stmt]);
        Ok(value_body)
    }

    fn match_type(&self,
                  _type_id: &RpTypeId,
                  data: &Statement,
                  kind: &RpMatchKind,
                  variable: &str,
                  decode: Statement,
                  result: Statement,
                  _value: &RpByTypeMatch)
                  -> Result<Elements> {
        let check = match *kind {
            RpMatchKind::Any => stmt!["true"],
            RpMatchKind::Object => stmt![&self.isinstance, "(", data, ", ", &self.dict, ")"],
            RpMatchKind::Array => stmt![&self.isinstance, "(", data, ", ", &self.list, ")"],
            RpMatchKind::String => stmt![&self.isinstance, "(", data, ", ", &self.basestring, ")"],
            RpMatchKind::Boolean => stmt![&self.isinstance, "(", data, ", ", &self.boolean, ")"],
            RpMatchKind::Number => stmt![&self.isinstance, "(", data, ", ", &self.number, ")"],
        };

        let mut value_body = Elements::new();

        value_body.push(stmt!["if ", check, ":"]);
        value_body.push_nested(stmt![&variable, " = ", decode]);
        value_body.push_nested(stmt!["return ", &result]);

        Ok(value_body)
    }
}
