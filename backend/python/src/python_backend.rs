use super::*;
use std::iter;
use std::rc::Rc;

const TYPE_SEP: &'static str = "_";

pub struct PythonBackend {
    pub env: Environment,
    listeners: Box<Listeners>,
    id_converter: Option<Box<Naming>>,
    to_lower_snake: Box<Naming>,
    staticmethod: BuiltInName,
    classmethod: BuiltInName,
    dict: BuiltInName,
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
            dict: Name::built_in("dict"),
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

    /// Build a function that raises an exception if the given value `stmt` is None.
    fn raise_if_none(&self, stmt: &Statement, field: &PythonField) -> Elements {
        let mut raise_if_none = Elements::new();
        let required_error = Variable::String(format!("{}: is a required field", field.name));

        raise_if_none.push(stmt!["if ", &stmt, " is None:"]);
        raise_if_none.push_nested(stmt!["raise Exception(", required_error, ")"]);

        raise_if_none
    }

    fn encode_method<E>(
        &self,
        name: &RpName,
        fields: &[Loc<PythonField>],
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

        fields.for_each_loc(|field| {
            let var_string = Variable::String(field.name.to_owned());
            let field_stmt = stmt!["self.", &field.ident];

            let value_stmt = self.dynamic_encode(name, &field.ty, &field_stmt)?;

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

            Ok(()) as Result<()>
        })?;

        encode_body.push(stmt!["return data"]);

        encode.push(encode_body.join(Spacing));
        Ok(encode)
    }

    fn encode_tuple_method(
        &self,
        name: &RpName,
        fields: &[Loc<PythonField>],
    ) -> Result<MethodSpec> {
        let mut values = Statement::new();

        let mut encode = MethodSpec::new("encode");
        encode.push_argument(stmt!["self"]);

        let mut encode_body = Elements::new();

        for field in fields {
            let stmt = stmt!["self.", &field.ident];
            encode_body.push(self.raise_if_none(&stmt, field));

            values.push(self.dynamic_encode(name, &field.ty, &stmt).with_pos(
                field.pos(),
            )?);
        }

        encode_body.push(stmt!["return (", values.join(", "), ")"]);
        encode.push(encode_body.join(Spacing));
        Ok(encode)
    }

    fn encode_enum_method(&self, field: &PythonField) -> Result<MethodSpec> {
        let mut encode = MethodSpec::new("encode");
        encode.push_argument(stmt!["self"]);

        let mut encode_body = Elements::new();

        encode_body.push(stmt!["return self.", &field.ident]);
        encode.push(encode_body.join(Spacing));
        Ok(encode)
    }

    fn decode_enum_method(&self, field: &PythonField) -> Result<MethodSpec> {
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

    fn repr_method<'a, I>(&self, name: &str, fields: I) -> MethodSpec
    where
        I: IntoIterator<Item = &'a Loc<PythonField<'a>>>,
    {
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
        name: &RpName,
        fields: &[Loc<PythonField>],
        variable_fn: F,
    ) -> Result<MethodSpec>
    where
        F: Fn(usize, &PythonField) -> Variable,
    {
        let data = stmt!["data"];

        let mut decode = MethodSpec::new("decode");
        decode.push_decorator(&self.staticmethod);
        decode.push_argument(&data);

        let mut decode_body = Elements::new();

        let mut arguments = Statement::new();

        for (i, field) in fields.iter().enumerate() {
            let var_name = format!("f_{}", field.ident);
            let var = variable_fn(i, field);

            let stmt = match *field.modifier {
                RpModifier::Optional => {
                    let var_name = var_name.clone().into();
                    let var_stmt = self.dynamic_decode(name, &field.ty, &var_name).with_pos(
                        field.pos(),
                    )?;
                    self.optional_check(&var_name, &var, &var_stmt)
                }
                _ => {
                    let var_stmt = stmt!["data[", &var, "]"];
                    let var_stmt = self.dynamic_decode(name, &field.ty, &var_stmt.into())
                        .with_pos(field.pos())?;
                    stmt![&var_name, " = ", &var_stmt].into()
                }
            };

            decode_body.push(stmt);
            arguments.push(var_name);
        }

        let arguments = arguments.join(", ");
        let name = self.convert_type(name)?;
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

    fn build_constructor<'a, I>(&self, fields: I) -> MethodSpec
    where
        I: IntoIterator<Item = &'a Loc<PythonField<'a>>>,
    {
        let mut constructor = MethodSpec::new("__init__");
        constructor.push_argument(stmt!["self"]);

        for field in fields {
            constructor.push_argument(stmt![&field.ident]);
            constructor.push(stmt!["self.", &field.ident, " = ", &field.ident]);
        }

        constructor
    }

    fn build_getters<'a, I>(&self, fields: I) -> Result<Vec<MethodSpec>>
    where
        I: IntoIterator<Item = &'a Loc<PythonField<'a>>>,
    {
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

    fn convert_type_id<F>(&self, name: &RpName, path_syntax: F) -> Result<Name>
    where
        F: Fn(Vec<&str>) -> String,
    {
        let registered = self.env.lookup(name)?;

        let local_name = registered.local_name(name, |p| p.join(TYPE_SEP), path_syntax);

        if let Some(ref used) = name.prefix {
            let package = self.package(&name.package).parts.join(".");
            return Ok(Name::imported_alias(&package, &local_name, used).into());
        }

        Ok(Name::local(&local_name).into())
    }

    pub fn enum_variants(&self, body: &RpEnumBody) -> Result<Statement> {
        let mut arguments = Statement::new();

        let variants = body.variants.iter().map(|l| l.loc_ref());

        variants.for_each_loc(|variant| {
            let var_name = Variable::String(variant.local_name.to_string());

            let mut enum_arguments = Statement::new();

            enum_arguments.push(var_name);
            enum_arguments.push(self.ordinal(variant)?);

            arguments.push(stmt!["(", enum_arguments.join(", "), ")"]);

            Ok(()) as Result<()>
        })?;

        let class_name = Variable::String(body.local_name.to_string());

        Ok(stmt![
            body.local_name.as_str(),
            " = ",
            &self.enum_enum,
            "(",
            class_name,
            ", [",
            arguments.join(", "),
            "], type=",
            body.local_name.as_str(),
            ")",
        ])
    }

    fn enum_ident(field: PythonField) -> PythonField {
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
    ) -> Loc<PythonField<'a>>
    where
        F: Fn(PythonField<'a>) -> PythonField<'a>,
    {
        let ident = self.field_ident(field);

        field.as_ref().map(|f| {
            python_field_f(PythonField {
                modifier: &f.modifier,
                ty: &f.ty,
                name: f.name(),
                ident: ident,
            })
        })
    }

    fn into_python_field<'a>(&self, field: &'a Loc<RpField>) -> Loc<PythonField<'a>> {
        self.into_python_field_with(field, |ident| ident)
    }

    fn as_class(&self, name: &RpName) -> ClassSpec {
        ClassSpec::new(name.join(TYPE_SEP).as_str())
    }

    pub fn process_tuple(
        &self,
        out: &mut PythonFileSpec,
        name: &RpName,
        body: Rc<Loc<RpTupleBody>>,
    ) -> Result<()> {
        let mut class = self.as_class(name);

        let fields: Vec<Loc<PythonField>> = body.fields
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
            class.push(code.take().lines);
        }

        let decode = self.decode_method(
            name,
            &fields,
            |i, _| Variable::Literal(i.to_string()),
        )?;
        class.push(decode);

        let encode = self.encode_tuple_method(&name, &fields)?;
        class.push(encode);

        let repr_method = self.repr_method(&class.name, &fields);
        class.push(repr_method);

        out.0.push(class);
        Ok(())
    }

    /// Process an enum for Python.
    pub fn process_enum(
        &self,
        out: &mut PythonFileSpec,
        name: &RpName,
        body: Rc<Loc<RpEnumBody>>,
    ) -> Result<()> {
        let mut class = self.as_class(name);

        let variant_field = body.variant_type.as_field();
        let field = Loc::new(variant_field, body.pos().clone());
        let field = self.into_python_field_with(&field, Self::enum_ident);

        class.push(self.build_constructor(iter::once(&field)));

        // TODO: make configurable
        if false {
            for getter in self.build_getters(iter::once(&field))? {
                class.push(&getter);
            }
        }

        for code in body.codes.for_context(PYTHON_CONTEXT) {
            class.push(code.take().lines);
        }

        class.push(self.encode_enum_method(&field)?);
        class.push(self.decode_enum_method(&field)?);

        let repr_method = self.repr_method(&class.name, iter::once(&field));
        class.push(repr_method);

        out.0.push(class);
        Ok(())
    }

    pub fn process_type(
        &self,
        out: &mut PythonFileSpec,
        name: &RpName,
        body: Rc<Loc<RpTypeBody>>,
    ) -> Result<()> {
        let mut class = self.as_class(name);

        let fields: Vec<Loc<PythonField>> = body.fields
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

        let decode = self.decode_method(name, &fields, |_, field| {
            Variable::String(field.name.to_owned())
        })?;

        class.push(decode);

        let encode = self.encode_method(name, &fields, &self.dict, |_| {})?;

        class.push(encode);

        let repr_method = self.repr_method(&class.name, &fields);
        class.push(repr_method);

        for code in body.codes.for_context(PYTHON_CONTEXT) {
            class.push(code.take().lines);
        }

        out.0.push(class);
        Ok(())
    }

    pub fn process_interface(
        &self,
        out: &mut PythonFileSpec,
        name: &RpName,
        body: Rc<Loc<RpInterfaceBody>>,
    ) -> Result<()> {
        let mut interface_spec = self.as_class(name);

        interface_spec.push(self.interface_decode_method(&body)?);

        for code in body.codes.for_context(PYTHON_CONTEXT) {
            interface_spec.push(code.take().lines);
        }

        let local_name = Name::local(&interface_spec.name);

        out.0.push(interface_spec);

        let values = body.sub_types.values().map(|l| l.loc_ref());

        values.for_each_loc(|sub_type| {
            let name = &sub_type.name;

            let mut class = self.as_class(&name);
            class.extends(&local_name);

            class.push(stmt![
                "TYPE = ",
                Variable::String(sub_type.name().to_owned()),
            ]);

            let fields: Vec<Loc<PythonField>> = body.fields
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

            let decode = self.decode_method(&name, &fields, |_, field| {
                Variable::String(field.ident.to_owned())
            })?;

            class.push(decode);

            let type_stmt =
                stmt![
                    "data[",
                    &self.type_var,
                    "] = ",
                    Variable::String(sub_type.name().to_owned()),
                ];

            let encode = self.encode_method(
                &name,
                &fields,
                &self.dict,
                move |elements| { elements.push(type_stmt); },
            )?;

            class.push(encode);

            let repr_method = self.repr_method(&class.name, &fields);
            class.push(repr_method);

            for code in sub_type.codes.for_context(PYTHON_CONTEXT) {
                class.push(code.take().lines);
            }

            out.0.push(class);

            Ok(()) as Result<()>
        })?;

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

    fn convert_type(&self, name: &RpName) -> Result<Name> {
        self.convert_type_id(name, |v| v.join(TYPE_SEP))
    }

    fn convert_constant(&self, name: &RpName) -> Result<Name> {
        // TODO: only last part in a constant lookup should be separated with dots.
        self.convert_type_id(name, |mut v| {
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
    fn string(&self, string: &str) -> Result<Self::Stmt> {
        Ok(Variable::String(string.to_owned()).into())
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
            Variable::String(name.value().to_owned()),
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
