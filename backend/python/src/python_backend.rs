use super::*;
use std::rc::Rc;

const TYPE_SEP: &'static str = "_";

pub struct PythonBackend {
    pub env: Environment,
    listeners: Box<Listeners>,
    id_converter: Option<Box<Naming>>,
    to_lower_snake: Box<Naming>,
    staticmethod: BuiltInName,
    classmethod: BuiltInName,
    isinstance: BuiltInName,
    dict: BuiltInName,
    list: BuiltInName,
    str_: BuiltInName,
    boolean: BuiltInName,
    number: ImportedName,
    enum_enum: ImportedName,
    type_var: Variable,
}

impl PythonBackend {
    pub fn new(
        env: Environment,
        _: PythonOptions,
        listeners: Box<Listeners>,
        id_converter: Option<Box<Naming>>,
    ) -> PythonBackend {
        PythonBackend {
            env: env,
            listeners: listeners,
            id_converter: id_converter,
            to_lower_snake: SnakeCase::new().to_lower_snake(),
            staticmethod: Name::built_in("staticmethod"),
            classmethod: Name::built_in("classmethod"),
            isinstance: Name::built_in("isinstance"),
            dict: Name::built_in("dict"),
            list: Name::built_in("list"),
            str_: Name::built_in("str"),
            boolean: Name::built_in("bool"),
            number: Name::imported("numbers", "Number"),
            enum_enum: Name::imported("enum", "Enum"),
            type_var: Variable::String(TYPE.to_owned()),
        }
    }

    pub fn compiler(&self, options: CompilerOptions) -> Result<PythonCompiler> {
        Ok(PythonCompiler {
            out_path: options.out_path,
            backend: self,
        })
    }

    pub fn verify(&self) -> Result<()> {
        Ok(())
    }

    fn find_field<'a>(
        &self,
        fields: &'a Vec<Loc<Field>>,
        name: &str,
    ) -> Option<(usize, &Field<'a>)> {
        for (i, field) in fields.iter().enumerate() {
            if field.name == name {
                return Some((i, field.as_ref()));
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

    fn encode_method<E>(
        &self,
        type_id: &RpTypeId,
        fields: &[Loc<Field>],
        builder: &BuiltInName,
        extra: E,
    ) -> Result<MethodSpec>
    where
        E: FnOnce(&mut Elements) -> (),
    {
        let mut encode = MethodSpec::new("encode");
        encode.push_argument(stmt!["self"]);

        let mut encode_body = Elements::new();

        encode_body.push(stmt!["data = ", builder, "()"]);

        extra(&mut encode_body);

        for field in fields {
            let var_string = Variable::String(field.name.to_owned());
            let field_stmt = stmt!["self.", &field.ident];
            let value_stmt = self.encode(type_id, field.pos(), &field.ty, &field_stmt)?;

            match *field.modifier {
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

    fn encode_tuple_method(&self, type_id: &RpTypeId, fields: &[Loc<Field>]) -> Result<MethodSpec> {
        let mut values = Statement::new();

        let mut encode = MethodSpec::new("encode");
        encode.push_argument(stmt!["self"]);

        let mut encode_body = Elements::new();

        for field in fields {
            let stmt = stmt!["self.", &field.ident];
            encode_body.push(self.raise_if_none(&stmt, field));
            values.push(self.encode(type_id, field.pos(), &field.ty, &stmt)?);
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

        member_loop.push(stmt![
            "for ",
            &value,
            " in ",
            &cls,
            ".__members__.values():",
        ]);
        member_loop.push_nested(check);

        let mismatch = Variable::String("data does not match enum".to_owned());
        let raise = stmt!["raise Exception(", mismatch, ")"];

        decode_body.push(member_loop);
        decode_body.push(raise);

        decode.push(decode_body.join(Spacing));
        Ok(decode)
    }

    fn repr_method(&self, name: &str, fields: &[Loc<Field>]) -> MethodSpec {
        let mut repr = MethodSpec::new("__repr__");
        repr.push_argument(stmt!["self"]);

        let mut arguments = Vec::new();
        let mut variables = Statement::new();

        for field in fields {
            arguments.push(format!("{}: {{!r}}", &field.ident));
            variables.push(stmt!["self.", &field.ident]);
        }

        let format = format!("<{} {}>", name, arguments.join(", "));
        repr.push(stmt![
            "return ",
            Variable::String(format),
            ".format(",
            variables.join(", "),
            ")",
        ]);

        repr
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

    fn decode_method<F>(
        &self,
        type_id: &RpTypeId,
        pos: &Pos,
        match_decl: &RpMatchDecl,
        fields: &[Loc<Field>],
        variable_fn: F,
    ) -> Result<MethodSpec>
    where
        F: Fn(usize, &Field) -> Variable,
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

            let stmt = match *field.modifier {
                RpModifier::Optional => {
                    let var_name = var_name.clone().into();
                    let var_stmt = self.decode(type_id, field.pos(), &field.ty, &var_name)?;
                    self.optional_check(&var_name, &var, &var_stmt)
                }
                _ => {
                    let var_stmt = stmt!["data[", &var, "]"];
                    let var_stmt = self.decode(
                        type_id,
                        field.pos(),
                        &field.ty,
                        &var_stmt.into(),
                    )?;
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

    fn field_ident(&self, field: &Loc<RpField>) -> String {
        self.ident(field.ident())
    }

    fn build_constructor(&self, fields: &[Loc<Field>]) -> MethodSpec {
        let mut constructor = MethodSpec::new("__init__");
        constructor.push_argument(stmt!["self"]);

        for field in fields {
            constructor.push_argument(stmt![&field.ident]);
            constructor.push(stmt!["self.", &field.ident, " = ", &field.ident]);
        }

        constructor
    }

    fn build_getters(&self, fields: &[Loc<Field>]) -> Result<Vec<MethodSpec>> {
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

    fn convert_type_id<F>(&self, pos: &Pos, lookup_id: &RpTypeId, path_syntax: F) -> Result<Name>
    where
        F: Fn(Vec<&str>) -> String,
    {
        let LookupResult {
            package,
            registered,
            type_id,
            ..
        } = self.env
            .lookup(&lookup_id.package, &lookup_id.name)
            .map_err(|e| Error::pos(e.description().to_owned(), pos.into()))?;

        let name = registered.local_name(&type_id, |p| p.join(TYPE_SEP), path_syntax);

        if let Some(ref used) = lookup_id.name.prefix {
            let package = self.package(package).parts.join(".");
            return Ok(Name::imported_alias(&package, &name, used).into());
        }

        Ok(Name::local(&name).into())
    }

    pub fn enum_variants(&self, type_id: &RpTypeId, body: &RpEnumBody) -> Result<Statement> {
        let mut arguments = Statement::new();

        let variables = Variables::new();

        for variant in &body.variants {
            let name = Variable::String((*variant.name).to_owned());

            let mut enum_arguments = Statement::new();

            enum_arguments.push(name);

            if !variant.arguments.is_empty() {
                let mut value_arguments = Statement::new();

                for (value, field) in variant.arguments.iter().zip(body.fields.iter()) {
                    let ctx =
                        ValueContext::new(&type_id.package, &variables, &value, Some(&field.ty));
                    value_arguments.push(self.value(ctx)?);
                }

                enum_arguments.push(stmt!["(", value_arguments.join(", "), ")"]);
            } else {
                enum_arguments.push(variant.ordinal.to_string());
            }

            arguments.push(stmt!["(", enum_arguments.join(", "), ")"]);
        }

        let class_name = Variable::String(body.name.to_owned());

        Ok(stmt![
            &body.name,
            " = ",
            &self.enum_enum,
            "(",
            class_name,
            ", [",
            arguments.join(", "),
            "], type=",
            &body.name,
            ")",
        ])
    }

    fn enum_ident(field: Field) -> Field {
        match field.ident.as_str() {
            "name" => field.with_ident("_name".to_owned()),
            "ordinal" => field.with_ident("_ordinal".to_owned()),
            _ => field,
        }
    }

    fn into_python_field_with<'a, F>(
        &self,
        field: &'a Loc<RpField>,
        python_field_f: F,
    ) -> Loc<Field<'a>>
    where
        F: Fn(Field<'a>) -> Field<'a>,
    {
        let ident = self.field_ident(field);

        field.map(|f| {
            python_field_f(Field {
                modifier: &f.modifier,
                ty: &f.ty,
                name: f.name(),
                ident: ident,
            })
        })
    }

    fn into_python_field<'a>(&self, field: &'a Loc<RpField>) -> Loc<Field<'a>> {
        self.into_python_field_with(field, |ident| ident)
    }

    fn as_class(&self, type_id: &RpTypeId) -> ClassSpec {
        ClassSpec::new(type_id.name.join(TYPE_SEP).as_str())
    }

    pub fn process_tuple(
        &self,
        out: &mut PythonFileSpec,
        type_id: &RpTypeId,
        pos: &Pos,
        body: Rc<RpTupleBody>,
    ) -> Result<()> {
        let mut class = self.as_class(type_id);

        let fields: Vec<Loc<Field>> = body.fields
            .iter()
            .map(|f| self.into_python_field(f))
            .collect();

        class.push(self.build_constructor(&fields));

        // TODO: make configurable
        if false {
            for getter in self.build_getters(&fields)? {
                class.push(&getter);
            }
        }

        for code in body.codes.for_context(PYTHON_CONTEXT) {
            class.push(code.move_inner().lines);
        }

        let decode = self.decode_method(
            type_id,
            pos,
            &body.match_decl,
            &fields,
            |i, _| Variable::Literal(i.to_string()),
        )?;
        class.push(decode);

        let encode = self.encode_tuple_method(&type_id, &fields)?;
        class.push(encode);

        let repr_method = self.repr_method(&class.name, &fields);
        class.push(repr_method);

        out.0.push(class);
        Ok(())
    }

    pub fn process_enum(
        &self,
        out: &mut PythonFileSpec,
        type_id: &RpTypeId,
        _: &Pos,
        body: Rc<RpEnumBody>,
    ) -> Result<()> {
        let mut class = self.as_class(type_id);

        let fields: Vec<Loc<Field>> = body.fields
            .iter()
            .map(|f| self.into_python_field_with(f, Self::enum_ident))
            .collect();

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
            class.push(code.move_inner().lines);
        }

        if let Some(ref s) = body.serialized_as {
            if let Some((_, ref field)) = self.find_field(&fields, s.as_ref()) {
                class.push(self.encode_enum_method(field)?);
                class.push(self.decode_enum_method(field)?);
            } else {
                return Err(Error::pos(format!("no field named: {}", s), s.pos().into()));
            }
        }

        let repr_method = self.repr_method(&class.name, &fields);
        class.push(repr_method);

        out.0.push(class);
        Ok(())
    }

    pub fn process_type(
        &self,
        out: &mut PythonFileSpec,
        type_id: &RpTypeId,
        pos: &Pos,
        body: Rc<RpTypeBody>,
    ) -> Result<()> {
        let mut class = self.as_class(type_id);

        let fields: Vec<Loc<Field>> = body.fields
            .iter()
            .map(|f| self.into_python_field(f))
            .collect();

        let constructor = self.build_constructor(&fields);
        class.push(&constructor);

        // TODO: make configurable
        if false {
            for getter in self.build_getters(&fields)? {
                class.push(getter);
            }
        }

        let decode = self.decode_method(
            type_id,
            pos,
            &body.match_decl,
            &fields,
            |_, field| Variable::String(field.name.to_owned()),
        )?;

        class.push(decode);

        let encode = self.encode_method(type_id, &fields, &self.dict, |_| {})?;

        class.push(encode);

        let repr_method = self.repr_method(&class.name, &fields);
        class.push(repr_method);

        for code in body.codes.for_context(PYTHON_CONTEXT) {
            class.push(code.move_inner().lines);
        }

        out.0.push(class);
        Ok(())
    }

    pub fn process_interface(
        &self,
        out: &mut PythonFileSpec,
        type_id: &RpTypeId,
        _: &Pos,
        body: Rc<RpInterfaceBody>,
    ) -> Result<()> {
        let mut interface_spec = self.as_class(type_id);

        interface_spec.push(self.interface_decode_method(type_id, &body)?);

        for code in body.codes.for_context(PYTHON_CONTEXT) {
            interface_spec.push(code.move_inner().lines);
        }

        let local_name = Name::local(&interface_spec.name);

        out.0.push(interface_spec);

        for (_, ref sub_type) in &body.sub_types {
            let type_id = type_id.extend(sub_type.name.clone());

            let mut class = self.as_class(&type_id);
            class.extends(&local_name);

            class.push(stmt![
                "TYPE = ",
                Variable::String(sub_type.name().to_owned()),
            ]);

            let fields: Vec<Loc<Field>> = body.fields
                .iter()
                .chain(sub_type.fields.iter())
                .map(|f| self.into_python_field(f))
                .collect();

            let constructor = self.build_constructor(&fields);
            class.push(&constructor);

            // TODO: make configurable
            if false {
                for getter in self.build_getters(&fields)? {
                    class.push(&getter);
                }
            }

            let decode = self.decode_method(
                &type_id,
                sub_type.pos(),
                &sub_type.match_decl,
                &fields,
                |_, field| Variable::String(field.ident.to_owned()),
            )?;

            class.push(decode);

            let type_stmt =
                stmt![
                "data[",
                &self.type_var,
                "] = ",
                Variable::String(sub_type.name().to_owned()),
            ];

            let encode = self.encode_method(
                &type_id,
                &fields,
                &self.dict,
                move |elements| { elements.push(type_stmt); },
            )?;

            class.push(encode);

            let repr_method = self.repr_method(&class.name, &fields);
            class.push(repr_method);

            for code in sub_type.codes.for_context(PYTHON_CONTEXT) {
                class.push(code.move_inner().lines);
            }

            out.0.push(class);
        }

        Ok(())
    }
}

impl PackageUtils for PythonBackend {}

impl Converter for PythonBackend {
    type Type = Name;
    type Stmt = Statement;
    type Elements = Elements;
    type Variable = Variable;

    fn new_var(&self, name: &str) -> Self::Stmt {
        stmt![name]
    }

    fn convert_type(&self, pos: &Pos, type_id: &RpTypeId) -> Result<Name> {
        self.convert_type_id(pos, type_id, |v| v.join(TYPE_SEP))
    }

    fn convert_constant(&self, pos: &Pos, type_id: &RpTypeId) -> Result<Name> {
        // TODO: only last part in a constant lookup should be separated with dots.
        self.convert_type_id(pos, type_id, |mut v| {
            let at = v.len().saturating_sub(2);
            let last = v.split_off(at).join(".");

            let mut parts = v.clone();
            parts.push(last.as_str());
            parts.join(TYPE_SEP)
        })
    }
}

/// Build values in python.
impl ValueBuilder for PythonBackend {
    fn env(&self) -> &Environment {
        &self.env
    }

    fn identifier(&self, identifier: &str) -> Result<Self::Stmt> {
        Ok(stmt![identifier])
    }

    fn optional_empty(&self) -> Result<Self::Stmt> {
        Ok(stmt!["None"])
    }

    fn optional_of(&self, value: Self::Stmt) -> Result<Self::Stmt> {
        Ok(value)
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

impl DynamicConverter for PythonBackend {
    fn is_native(&self, ty: &RpType) -> bool {
        match *ty {
            RpType::Signed { size: _ } |
            RpType::Unsigned { size: _ } => true,
            RpType::Float | RpType::Double => true,
            RpType::String => true,
            RpType::Any => true,
            RpType::Boolean => true,
            RpType::Array { ref inner } => self.is_native(inner),
            RpType::Map { ref key, ref value } => self.is_native(key) && self.is_native(value),
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

impl DynamicDecode for PythonBackend {
    type Method = MethodSpec;

    fn name_decode(&self, input: &Statement, name: Self::Type) -> Self::Stmt {
        stmt![name, ".decode(", input, ")"]
    }

    fn array_decode(&self, input: &Statement, inner: Statement) -> Self::Stmt {
        stmt!["map(lambda v: ", inner, ", ", input, ")"]
    }

    fn map_decode(&self, input: &Statement, key: Statement, value: Statement) -> Self::Stmt {
        let body = stmt!["(", &key, ", ", &value, ")"];
        stmt![
            &self.dict,
            "(map(lambda t: ",
            &body,
            ", ",
            input,
            ".items()))",
        ]
    }

    fn assign_type_var(&self, data: &Self::Stmt, type_var: &Self::Stmt) -> Self::Stmt {
        stmt![type_var, " = ", data, "[", &self.type_var, "]"]
    }

    fn check_type_var(
        &self,
        _data: &Self::Stmt,
        type_var: &Self::Stmt,
        name: &Loc<String>,
        type_name: &Self::Type,
    ) -> Self::Elements {
        let mut check = Elements::new();
        check.push(stmt![
            "if ",
            type_var,
            " == ",
            Variable::String(name.as_ref().to_owned()),
            ":",
        ]);
        check.push_nested(stmt!["return ", type_name, ".decode(data)"]);
        check
    }

    fn raise_bad_type(&self, type_var: &Self::Stmt) -> Self::Stmt {
        stmt![
            "raise Exception(",
            Variable::String("bad type".to_owned()),
            " + ",
            type_var,
            ")",
        ]
    }

    fn new_decode_method(&self, data: &Self::Stmt, body: Self::Elements) -> Self::Method {
        let mut decode = MethodSpec::new("decode");
        decode.push_decorator(&self.staticmethod);
        decode.push_argument(&data);
        decode.push(body);
        decode
    }
}

impl DynamicEncode for PythonBackend {
    fn name_encode(&self, input: &Statement, _: Self::Type) -> Self::Stmt {
        stmt![input, ".encode()"]
    }

    fn array_encode(&self, input: &Statement, inner: Statement) -> Self::Stmt {
        stmt!["map(lambda v: ", inner, ", ", input, ")"]
    }

    fn map_encode(&self, input: &Statement, key: Statement, value: Statement) -> Self::Stmt {
        let body = stmt!["(", &key, ", ", &value, ")"];
        stmt![
            &self.dict,
            "(",
            input,
            ".items().map(lambda t: ",
            &body,
            "))",
        ]
    }
}

impl MatchDecode for PythonBackend {
    fn match_value(
        &self,
        data: &Statement,
        _value: &RpValue,
        value_stmt: Statement,
        _result: &RpObject,
        result_stmt: Statement,
    ) -> Result<Elements> {
        let mut value_body = Elements::new();
        value_body.push(stmt!["if ", &data, " == ", value_stmt, ":"]);
        value_body.push_nested(stmt!["return ", result_stmt]);
        Ok(value_body)
    }

    fn match_type(
        &self,
        _type_id: &RpTypeId,
        data: &Statement,
        kind: &RpMatchKind,
        variable: &str,
        decode: Statement,
        result: Statement,
        _value: &RpByTypeMatch,
    ) -> Result<Elements> {
        let check = match *kind {
            RpMatchKind::Any => stmt!["true"],
            RpMatchKind::Object => stmt![&self.isinstance, "(", data, ", ", &self.dict, ")"],
            RpMatchKind::Array => stmt![&self.isinstance, "(", data, ", ", &self.list, ")"],
            RpMatchKind::String => stmt![&self.isinstance, "(", data, ", ", &self.str_, ")"],
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
