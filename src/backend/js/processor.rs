use backend::*;
use backend::collecting::Collecting;
use backend::errors::*;
use backend::for_context::ForContext;
use backend::package_processor::PackageProcessor;
use backend::variables::Variables;
use codeviz::js::*;
use naming::{self, FromNaming};
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use super::converter::Converter;
use super::dynamic_converter::DynamicConverter;
use super::dynamic_decode::DynamicDecode;
use super::dynamic_encode::DynamicEncode;
use super::match_decode::MatchDecode;
use super::models::*;
use super::utils::*;
use super::value_builder::*;

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
    pub build_getters: bool,
    pub build_constructor: bool,
}

impl ProcessorOptions {
    pub fn new() -> ProcessorOptions {
        ProcessorOptions {
            build_getters: false,
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
    type_var: Variable,
    values: Statement,
    enum_ordinal: Variable,
    enum_name: Variable,
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
            type_var: string(TYPE),
            values: stmt!["values"],
            enum_ordinal: Variable::Literal("ordinal".to_owned()),
            enum_name: Variable::Literal("name".to_owned()),
        }
    }

    fn find_field<'a>(&self,
                      fields: &'a Vec<RpLoc<JsField>>,
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

    fn convert_fields(&self, fields: &Vec<RpLoc<RpField>>) -> Vec<RpLoc<JsField>> {
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
                           type_id: &RpTypeId,
                           fields: &Vec<RpLoc<JsField>>,
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
            let value_stmt = self.encode(type_id, &field.pos, &field.ty, &field_stmt)?;

            match field.modifier {
                RpModifier::Optional => {
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
            body.push(assign.join(Spacing));
        }

        body.push(js![return data]);

        encode.push(body.join(Spacing));
        Ok(encode)
    }

    fn encode_tuple_method(&self,
                           type_id: &RpTypeId,
                           fields: &Vec<RpLoc<JsField>>)
                           -> Result<MethodSpec> {
        let mut values = Statement::new();

        let mut encode = MethodSpec::new("encode");

        let mut encode_body = Elements::new();

        for field in fields {
            let stmt = stmt!["this.", &field.ident];
            encode_body.push(self.throw_if_null(&stmt, field));
            values.push(self.encode(type_id, &field.pos, &field.ty, &stmt)?);
        }

        encode_body.push(js![@return [ values ]]);
        encode.push(encode_body.join(Spacing));
        Ok(encode)
    }

    fn encode_enum_method(&self, ident: &str) -> Result<MethodSpec> {
        let mut encode = MethodSpec::new("encode");
        let mut encode_body = Elements::new();
        encode_body.push(js![return "this.", &ident]);
        encode.push(encode_body.join(Spacing));
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
        member_loop.push(js![for loop_init; stmt![&i, " < ", &l]; stmt![&i, "++"], body.join(Spacing)]);

        let mut body = Elements::new();

        body.push(member_loop);
        body.push(js![throw string("no matching value")]);

        decode.push(body.join(Spacing));
        Ok(decode)
    }

    fn decode_method<F>(&self,
                        type_id: &RpTypeId,
                        match_decl: &RpMatchDecl,
                        fields: &Vec<RpLoc<JsField>>,
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

            let stmt: Element = match field.modifier {
                RpModifier::Optional => {
                    let var_name = var_name.clone().into();
                    let var_stmt = self.decode(type_id, &field.pos, &field.ty, &var_name)?;

                    let mut check = Elements::new();

                    check.push(stmt!["let ", &var_name, " = ", &data, "[", &var, "];"]);
                    check.push(Spacing);
                    check.push(js![if is_defined(stmt![&var_name]),
                                      stmt![&var_name, " = ", var_stmt, ";"],
                                      stmt![&var_name, " = null", ";"]]);

                    check.into()
                }
                _ => {
                    let var_stmt = stmt![&data, "[", &var, "]"];
                    let var_stmt = self.decode(type_id, &field.pos, &field.ty, &var_stmt.into())?;
                    stmt!["const ", &var_name, " = ", &var_stmt, ";"].into()
                }
            };

            assign.push(stmt);
            arguments.push(var_name);
        }

        let mut body = Elements::new();

        if let Some(by_value) = self.decode_by_value(type_id, match_decl, &data)? {
            body.push(by_value.join(Spacing));
        }

        if let Some(by_type) = self.decode_by_type(type_id, match_decl, &data)? {
            body.push(by_type.join(Spacing));
        }

        if !assign.is_empty() {
            body.push(assign.join(Spacing));
        }

        body.push(js![@return new &class.name, arguments]);

        decode.push(body.join(Spacing));

        Ok(decode)
    }

    fn field_ident(&self, field: &RpField) -> String {
        if let Some(ref id_converter) = self.id_converter {
            id_converter.convert(&field.name)
        } else {
            field.name.to_owned()
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

    fn build_constructor(&self, fields: &Vec<RpLoc<JsField>>) -> ConstructorSpec {
        let mut ctor = ConstructorSpec::new();
        let mut assignments = Elements::new();

        for field in fields {
            ctor.push_argument(stmt![&field.ident]);
            assignments.push(stmt!["this.", &field.ident, " = ", &field.ident, ";"]);
        }

        ctor.push(assignments);
        ctor
    }

    fn build_enum_constructor(&self, fields: &Vec<RpLoc<JsField>>) -> ConstructorSpec {
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

    fn enum_encode_decode(&self,
                          body: &RpEnumBody,
                          fields: &Vec<RpLoc<JsField>>,
                          class: &ClassSpec)
                          -> Result<Element> {
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

    fn build_getters(&self, fields: &Vec<RpLoc<JsField>>) -> Result<Vec<MethodSpec>> {
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
                     _: &RpPos,
                     body: Rc<RpTupleBody>)
                     -> Result<()> {
        let mut class = ClassSpec::new(&body.name);
        let mut fields: Vec<RpLoc<JsField>> = Vec::new();

        for field in &body.fields {
            let ident = self.field_ident(&field);

            fields.push(field.clone()
                .map_inner(|f| {
                    JsField {
                        modifier: RpModifier::Required,
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

        let decode = self.decode_method(type_id, &body.match_decl, &fields, &class, field_index)?;
        class.push(decode);

        let encode = self.encode_tuple_method(type_id, &fields)?;
        class.push(encode);

        for code in body.codes.for_context(JS_CONTEXT) {
            class.push(code.inner.lines);
        }

        out.push(class);
        Ok(())
    }

    fn process_enum(&self,
                    out: &mut Self::Out,
                    type_id: &RpTypeId,
                    _: &RpPos,
                    body: Rc<RpEnumBody>)
                    -> Result<()> {
        let mut class = ClassSpec::new(&body.name);
        let mut fields: Vec<RpLoc<JsField>> = Vec::new();

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
                        modifier: RpModifier::Required,
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
        let variables = Variables::new();

        for variant in &body.variants {
            let mut value_arguments = Statement::new();

            value_arguments.push(variant.ordinal.to_string());
            value_arguments.push(string(&*variant.name));

            for (value, field) in variant.arguments.iter().zip(fields.iter()) {
                let env = ValueBuilderEnv {
                    value: &value,
                    package: &type_id.package,
                    ty: Some(&field.ty),
                    variables: &variables,
                };

                value_arguments.push(self.value(&env)?);
            }

            let arguments = js![new &body.name, value_arguments];
            let member = stmt![&class.name, ".", &*variant.name];

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

        out.push(elements.join(Spacing));
        Ok(())
    }


    fn process_type(&self,
                    out: &mut Self::Out,
                    type_id: &RpTypeId,
                    _: &RpPos,
                    body: Rc<RpTypeBody>)
                    -> Result<()> {
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

        let decode = self.decode_method(type_id, &body.match_decl, &fields, &class, field_ident)?;
        class.push(decode);

        let encode = self.encode_method(type_id, &fields, "{}", |_| {})?;
        class.push(encode);

        for code in body.codes.for_context(JS_CONTEXT) {
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
        let mut classes = Elements::new();

        let mut interface_spec = ClassSpec::new(&body.name);

        interface_spec.push(self.interface_decode_method(type_id, &body)?);

        let interface_fields = self.convert_fields(&body.fields);

        for code in body.codes.for_context(JS_CONTEXT) {
            interface_spec.push(code.inner.lines);
        }

        classes.push(interface_spec);

        for (_, ref sub_type) in &body.sub_types {
            let mut class = ClassSpec::new(&format!("{}_{}", &body.name, &sub_type.name));

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

            let decode =
                self.decode_method(type_id, &sub_type.match_decl, &fields, &class, field_ident)?;

            class.push(decode);

            let type_stmt = stmt!["data[", &self.type_var, "] = ", &class.name, ".TYPE;"];

            let encode = self.encode_method(type_id, &fields, "{}", move |elements| {
                    elements.push(type_stmt);
                })?;

            class.push(encode);

            for code in sub_type.codes.for_context(JS_CONTEXT) {
                class.push(code.inner.lines);
            }

            classes.push(&class);
            classes.push(stmt![&class.name, ".TYPE", " = ", string(sub_type.name.clone()), ";"]);
        }

        out.push(classes.join(Spacing));
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
        let name = &type_id.name;

        if let Some(ref used) = name.prefix {
            let package = self.env
                .lookup_used(&type_id.package, used)
                .map_err(|e| Error::pos(e.description().to_owned(), pos.clone()))?;

            let package = self.package(package);
            let package = package.parts.join(".");
            return Ok(Name::imported_alias(&package, &name.parts.join("."), used).into());
        }

        Ok(Name::local(&name.parts.join("_")).into())
    }
}

/// Build values in js.
impl ValueBuilder for Processor {
    fn env(&self) -> &Environment {
        &self.env
    }

    fn identifier(&self, identifier: &str) -> Result<Self::Stmt> {
        Ok(stmt![identifier])
    }

    fn optional_empty(&self) -> Result<Self::Stmt> {
        Ok(stmt!["null"])
    }

    fn constant(&self, ty: Self::Type) -> Result<Self::Stmt> {
        return Ok(stmt![ty]);
    }

    fn instance(&self, ty: Self::Type, arguments: Vec<Self::Stmt>) -> Result<Self::Stmt> {
        let mut stmt = Statement::new();

        for a in arguments {
            stmt.push(a);
        }

        Ok(stmt!["new ", &ty, "(", stmt.join(", "), ")"])
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
        stmt![input, ".map(function(v) { ", inner, "; })"]
    }

    fn map_decode(&self, input: &Statement, key: Statement, value: Statement) -> Self::Stmt {
        let body = stmt!["[", &key, ", ", &value, "]"];
        // TODO: these functions need to be implemented and hoisted into the module
        stmt!["to_object(from_object(", input, ").map(function(t) { return ", &body, "; }))"]
    }

    fn assign_type_var(&self, data: &Self::Stmt, type_var: &Self::Stmt) -> Self::Stmt {
        stmt!["const ", type_var, " = ", data, "[", &self.type_var, "]"]
    }

    fn check_type_var(&self,
                      data: &Self::Stmt,
                      type_var: &Self::Stmt,
                      name: &RpLoc<String>,
                      type_name: &Self::Type)
                      -> Self::Elements {
        let mut body = Elements::new();
        let cond = stmt![type_var, " === ", string(&name.inner)];
        body.push(js![if cond, js![return type_name, ".decode(", &data, ")"]]);
        body
    }

    fn raise_bad_type(&self, type_var: &Self::Stmt) -> Self::Stmt {
        js![throw string("bad type: "), " + ", type_var]
    }

    fn new_decode_method(&self, data: &Self::Stmt, body: Self::Elements) -> Self::Method {
        let mut decode = MethodSpec::with_static("decode");
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
        stmt![input, ".map(function(v) { return ", inner, "; })"]
    }

    fn map_encode(&self, input: &Statement, key: Statement, value: Statement) -> Self::Stmt {
        let body = stmt!["[", &key, ", ", &value, "]"];
        // TODO: these functions need to be implemented and hoisted into the module
        stmt!["to_object(from_object(", input, ").map(function(t) { return ", &body, "; }))"]
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
        value_body.push(stmt!["if (", data, " == ", value_stmt, ") {"]);
        value_body.push_nested(stmt!["return ", result_stmt]);
        value_body.push("}");
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
            RpMatchKind::Object => stmt![data, ".constructor === Object"],
            RpMatchKind::Array => stmt![data, ".constructor === Array"],
            RpMatchKind::String => stmt!["typeof ", data, " === \"string\""],
            RpMatchKind::Boolean => stmt!["typeof ", data, " === \"boolean\""],
            RpMatchKind::Number => stmt!["typeof ", data, " === \"number\""],
        };

        let mut value_body = Elements::new();

        value_body.push(stmt!["if (", check, ") {"]);
        value_body.push_nested(stmt![&variable, " = ", decode]);
        value_body.push_nested(stmt!["return ", &result, ";"]);
        value_body.push("}");

        Ok(value_body)
    }
}
