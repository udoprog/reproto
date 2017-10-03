use super::*;
use std::rc::Rc;

const TYPE_SEP: &'static str = "_";

pub struct JsBackend {
    pub env: Environment,
    listeners: Box<Listeners>,
    id_converter: Option<Box<Naming>>,
    to_lower_snake: Box<Naming>,
    type_var: Variable,
    values: Statement,
    enum_name: Variable,
}

impl JsBackend {
    pub fn new(
        env: Environment,
        _: JsOptions,
        listeners: Box<Listeners>,
        id_converter: Option<Box<Naming>>,
    ) -> JsBackend {
        JsBackend {
            env: env,
            listeners: listeners,
            id_converter: id_converter,
            to_lower_snake: SnakeCase::new().to_lower_snake(),
            type_var: string(TYPE),
            values: stmt!["values"],
            enum_name: Variable::Literal("name".to_owned()),
        }
    }

    pub fn compiler(&self, options: CompilerOptions) -> Result<JsCompiler> {
        Ok(JsCompiler {
            out_path: options.out_path,
            backend: self,
        })
    }

    pub fn verify(&self) -> Result<()> {
        Ok(())
    }

    /// Build a function that throws an exception if the given value `stmt` is None.
    fn throw_if_null<S>(&self, stmt: S, field: &JsField) -> Elements
    where
        S: Into<Statement>,
    {
        let required_error = string(format!("{}: is a required field", field.name));
        js![if is_not_defined(stmt), js![throw required_error]]
    }

    fn encode_method<E, B>(
        &self,
        type_id: &RpName,
        fields: &[Loc<JsField>],
        builder: B,
        extra: E,
    ) -> Result<MethodSpec>
    where
        E: FnOnce(&mut Elements) -> (),
        B: Into<Variable>,
    {
        let mut encode = MethodSpec::new("encode");
        let mut body = Elements::new();
        let data = stmt!["data"];

        body.push(stmt!["const ", &data, " = ", builder, ";"]);

        extra(&mut body);

        let mut assign = Elements::new();

        fields.for_each_loc(|field| {
            let var_string = string(field.name.to_owned());
            let field_stmt = stmt!["this.", &field.ident];
            let value_stmt = self.dynamic_encode(type_id, &field.ty, &field_stmt)?;

            match *field.modifier {
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

            Ok(()) as Result<()>
        })?;

        if !assign.is_empty() {
            body.push(assign.join(Spacing));
        }

        body.push(js![return data]);

        encode.push(body.join(Spacing));
        Ok(encode)
    }

    fn encode_tuple_method(&self, type_id: &RpName, fields: &[Loc<JsField>]) -> Result<MethodSpec> {
        let mut values = Statement::new();

        let mut encode = MethodSpec::new("encode");

        let mut encode_body = Elements::new();

        fields.for_each_loc(|field| {
            let stmt = stmt!["this.", &field.ident];
            encode_body.push(self.throw_if_null(stmt.clone(), field));
            values.push(self.dynamic_encode(type_id, &field.ty, &stmt)?);
            Ok(()) as Result<()>
        })?;

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

        decode.push_argument("data");

        let mut member_loop = Elements::new();

        let mut body = Elements::new();

        let members = stmt![&class.name, ".", &self.values];
        body.push(js![const &member, &members, "[", &i, "]"]);

        let cond = stmt![&member, ".", ident, " === ", &data];
        body.push(js![if cond, js![return &member]]);

        let loop_init = stmt!["let ", &i, " = 0, ", &l, " = ", &members, ".length"];
        member_loop.push(
            js![for loop_init; stmt![&i, " < ", &l]; stmt![&i, "++"], body.join(Spacing)],
        );

        let mut body = Elements::new();

        body.push(member_loop);
        body.push(
            js![throw string("no matching value: "), stmt![" + "], &data],
        );

        decode.push(body.join(Spacing));
        Ok(decode)
    }

    fn decode_method<F>(
        &self,
        type_id: &RpName,
        fields: &[Loc<JsField>],
        class: &ClassSpec,
        variable_fn: F,
    ) -> Result<MethodSpec>
    where
        F: Fn(usize, &JsField) -> Variable,
    {
        let mut decode = MethodSpec::with_static("decode");
        let data = stmt!["data"];

        decode.push_argument(data.clone());

        let mut arguments = Statement::new();
        let mut assign = Elements::new();

        for (i, field) in fields.iter().enumerate() {
            let var_name = format!("v_{}", field.ident.clone());
            let var = variable_fn(i, field);

            let stmt = field.as_ref().and_then(|field| match *field.modifier {
                RpModifier::Optional => {
                    let var_name = var_name.clone().into();
                    let var_stmt = self.dynamic_decode(type_id, &field.ty, &var_name)?;

                    let mut check = Elements::new();

                    check.push(stmt!["let ", &var_name, " = ", &data, "[", &var, "];"]);
                    check.push(Spacing);
                    check.push(js![if is_defined(stmt![&var_name]),
                                      stmt![&var_name, " = ", var_stmt, ";"],
                                      stmt![&var_name, " = null", ";"]]);

                    Ok(check.into()) as Result<Element>
                }
                _ => {
                    let var_stmt = stmt![&data, "[", &var, "]"];
                    let var_stmt = self.dynamic_decode(type_id, &field.ty, &var_stmt.into())?;

                    let mut check = Elements::new();

                    check.push(stmt!["const ", &var_name, " = ", &var_stmt, ";"]);
                    check.push(Spacing);
                    check.push(js![if is_not_defined(stmt![&var_name]),
                                   js![throw &var, " + ", string(": required field")]]);

                    Ok(check.into()) as Result<Element>
                }
            })?;

            assign.push(stmt);
            arguments.push(var_name);
        }

        let mut body = Elements::new();

        if !assign.is_empty() {
            body.push(assign.join(Spacing));
        }

        body.push(js![@return new &class.name, arguments]);

        decode.push(body.join(Spacing));

        Ok(decode)
    }

    fn field_by_name(_i: usize, field: &JsField) -> Variable {
        string(&field.name)
    }

    fn field_by_index(i: usize, _field: &JsField) -> Variable {
        Variable::Literal(i.to_string())
    }

    fn field_ident(&self, field: &RpField) -> String {
        if let Some(ref id_converter) = self.id_converter {
            id_converter.convert(field.ident())
        } else {
            field.ident().to_owned()
        }
    }

    fn build_constructor(&self, fields: &[Loc<JsField>]) -> ConstructorSpec {
        let mut ctor = ConstructorSpec::new();
        let mut assignments = Elements::new();

        for field in fields {
            ctor.push_argument(stmt![&field.ident]);
            assignments.push(stmt!["this.", &field.ident, " = ", &field.ident, ";"]);
        }

        ctor.push(assignments);
        ctor
    }

    fn build_enum_constructor(&self, field: &JsField) -> ConstructorSpec {
        let mut ctor = ConstructorSpec::new();
        let mut assignments = Elements::new();

        ctor.push_argument(self.enum_name.clone());
        assignments.push(stmt!["this.", &self.enum_name, " = ", &self.enum_name, ";"]);

        ctor.push_argument(stmt![&field.ident]);
        assignments.push(stmt!["this.", &field.ident, " = ", &field.ident, ";"]);

        ctor.push(assignments);
        ctor
    }

    fn enum_encode_decode(&self, field: &Loc<JsField>, class: &ClassSpec) -> Result<Element> {
        let mut elements = Elements::new();
        elements.push(self.encode_enum_method(&field.name)?);
        let decode = self.decode_enum_method(&class, &field.name)?;
        elements.push(decode);
        return Ok(elements.into());
    }

    fn build_getters(&self, fields: &[Loc<JsField>]) -> Result<Vec<MethodSpec>> {
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

    fn any_ident(field: JsField) -> JsField {
        field
    }

    fn enum_ident(field: JsField) -> JsField {
        match field.ident.as_str() {
            "name" => field.with_ident("_name".to_owned()),
            _ => field,
        }
    }

    fn into_js_field_with<'b, F>(&self, field: &'b Loc<RpField>, js_field_f: F) -> Loc<JsField<'b>>
    where
        F: Fn(JsField<'b>) -> JsField<'b>,
    {
        let ident = self.field_ident(&field);

        field.as_ref().map(|f| {
            js_field_f(JsField {
                modifier: &f.modifier,
                ty: &f.ty,
                name: f.name(),
                ident: ident,
            })
        })
    }

    fn into_js_field<'b>(&self, field: &'b Loc<RpField>) -> Loc<JsField<'b>> {
        self.into_js_field_with(field, Self::any_ident)
    }

    pub fn process_tuple(
        &self,
        out: &mut JsFileSpec,
        name: &RpName,
        body: Rc<Loc<RpTupleBody>>,
    ) -> Result<()> {
        let mut class = ClassSpec::new(&name.join(TYPE_SEP));
        class.export();

        let fields: Vec<Loc<JsField>> = body.fields.iter().map(|f| self.into_js_field(f)).collect();

        class.push(self.build_constructor(&fields));

        // TODO: make configurable
        if false {
            for getter in self.build_getters(&fields)? {
                class.push(&getter);
            }
        }

        let decode = self.decode_method(
            name,
            &fields,
            &class,
            Self::field_by_index,
        )?;

        class.push(decode);

        let encode = self.encode_tuple_method(name, &fields)?;
        class.push(encode);

        for code in body.codes.for_context(JS_CONTEXT) {
            class.push(code.take().lines);
        }

        out.0.push(class);
        Ok(())
    }

    /// Convert enum to JavaScript.
    pub fn process_enum(
        &self,
        out: &mut JsFileSpec,
        name: &RpName,
        body: Rc<Loc<RpEnumBody>>,
    ) -> Result<()> {
        let mut class = ClassSpec::new(&name.join(TYPE_SEP));
        class.export();

        let variant_field = Loc::new(body.variant_type.as_field(), body.pos().clone());
        let field = self.into_js_field_with(&variant_field, Self::enum_ident);

        let mut members = Statement::new();

        class.push(self.build_enum_constructor(&field));
        let encode_decode = self.enum_encode_decode(&field, &class)?;
        class.push(encode_decode);

        let mut values = Elements::new();

        body.variants.for_each_loc(|variant| {
            let mut arguments = Statement::new();

            arguments.push(string(variant.local_name.as_str()));
            arguments.push(self.ordinal(variant)?);

            let arguments = js![new & body.local_name, arguments];
            let member = stmt![&class.name, ".", variant.local_name.as_str()];

            values.push(js![= &member, arguments]);
            members.push(member);

            Ok(()) as Result<()>
        })?;

        for code in body.codes.for_context(JS_CONTEXT) {
            class.push(code.take().lines);
        }

        let mut elements = Elements::new();

        // class declaration
        elements.push(&class);

        // enum literal values
        elements.push(values);

        // push members field
        let members_key = stmt![&class.name, ".", &self.values];
        elements.push(js![= members_key, js!([members])]);

        out.0.push(elements.join(Spacing));
        Ok(())
    }


    pub fn process_type(
        &self,
        out: &mut JsFileSpec,
        name: &RpName,
        body: Rc<Loc<RpTypeBody>>,
    ) -> Result<()> {
        let fields: Vec<_> = body.fields.iter().map(|f| self.into_js_field(f)).collect();

        let mut class = ClassSpec::new(&name.join(TYPE_SEP));
        class.export();

        let constructor = self.build_constructor(&fields);
        class.push(&constructor);

        // TODO: make configurable
        if false {
            for getter in self.build_getters(&fields)? {
                class.push(getter);
            }
        }

        let decode = self.decode_method(
            name,
            &fields,
            &class,
            Self::field_by_name,
        )?;
        class.push(decode);

        let encode = self.encode_method(name, &fields, "{}", |_| {})?;
        class.push(encode);

        for code in body.codes.for_context(JS_CONTEXT) {
            class.push(code.take().lines);
        }

        out.0.push(class);
        Ok(())
    }

    pub fn process_interface(
        &self,
        out: &mut JsFileSpec,
        name: &RpName,
        body: Rc<Loc<RpInterfaceBody>>,
    ) -> Result<()> {
        let mut classes = Elements::new();

        let mut interface_spec = ClassSpec::new(&name.join(TYPE_SEP));
        interface_spec.export();

        interface_spec.push(self.interface_decode_method(&body)?);

        let interface_fields: Vec<Loc<JsField>> =
            body.fields.iter().map(|f| self.into_js_field(f)).collect();

        for code in body.codes.for_context(JS_CONTEXT) {
            interface_spec.push(code.take().lines);
        }

        classes.push(interface_spec);

        let sub_types = body.sub_types.values().map(|l| l.loc_ref());

        sub_types.for_each_loc(|sub_type| {
            let mut class = ClassSpec::new(&format!("{}_{}", &body.name, &sub_type.name));
            class.export();

            let fields: Vec<_> = interface_fields
                .clone()
                .into_iter()
                .chain(sub_type.fields.iter().map(|f| self.into_js_field(f)))
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
                name,
                &fields,
                &class,
                Self::field_by_name,
            )?;

            class.push(decode);

            let type_stmt = stmt!["data[", &self.type_var, "] = ", &class.name, ".TYPE;"];

            let encode = self.encode_method(name, &fields, "{}", move |elements| {
                elements.push(type_stmt);
            })?;

            class.push(encode);

            for code in sub_type.codes.for_context(JS_CONTEXT) {
                class.push(code.take().lines);
            }

            classes.push(&class);
            classes.push(stmt![
                &class.name,
                ".TYPE",
                " = ",
                string(sub_type.name.clone()),
                ";",
            ]);

            Ok(()) as Result<()>
        })?;

        out.0.push(classes.join(Spacing));
        Ok(())
    }
}

impl PackageUtils for JsBackend {}

impl Converter for JsBackend {
    type Type = Name;
    type Stmt = Statement;
    type Elements = Elements;
    type Variable = Variable;

    fn new_var(&self, name: &str) -> Self::Stmt {
        stmt![name]
    }

    fn convert_type(&self, name: &RpName) -> Result<Name> {
        let registered = self.env.lookup(name)?;

        let local_name = registered.local_name(name, |p| p.join(TYPE_SEP), |c| c.join(TYPE_SEP));

        if let Some(ref used) = name.prefix {
            let package = self.package(&name.package).parts.join(".");
            return Ok(Name::imported_alias(&package, &local_name, used).into());
        }

        Ok(Name::local(&local_name).into())
    }
}

/// Build values in js.
impl ValueBuilder for JsBackend {
    fn string(&self, string: &str) -> Result<Self::Stmt> {
        Ok(Variable::String(string.to_owned()).into())
    }
}

impl DynamicConverter for JsBackend {
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
        stmt!["k"]
    }

    fn map_value_var(&self) -> Statement {
        stmt!["data[k]"]
    }

    fn array_inner_var(&self) -> Statement {
        stmt!["v"]
    }
}

impl DynamicDecode for JsBackend {
    type Method = MethodSpec;

    fn name_decode(&self, input: &Statement, name: Self::Type) -> Self::Stmt {
        stmt![name, ".decode(", input, ")"]
    }

    /// Decoding an Array in JavaScript.
    ///
    /// Maps over each decoded value using `Array.map(...)`, decoding each variable.
    fn array_decode(&self, input: &Statement, inner: Statement) -> Self::Stmt {
        stmt![input, ".map(function(v) { return ", inner, "; })"]
    }

    /// Decoding a map in JavaScript.
    fn map_decode(&self, input: &Statement, _key: Statement, value: Statement) -> Self::Stmt {
        let mut elements = Statement::new();

        elements.push("(function(data) {");
        elements.push(" let o = {};");
        elements.push(" for (let k in data) {");
        elements.push(stmt![" o[k] = ", &value, ";"]);
        elements.push(" };");
        elements.push(" return o;");
        elements.push(stmt![" })(", input, ")"]);

        elements
    }

    fn assign_type_var(&self, data: &Self::Stmt, type_var: &Self::Stmt) -> Self::Stmt {
        stmt!["const ", type_var, " = ", data, "[", &self.type_var, "]"]
    }

    fn check_type_var(
        &self,
        data: &Self::Stmt,
        type_var: &Self::Stmt,
        name: &Loc<String>,
        type_name: &Self::Type,
    ) -> Self::Elements {
        let mut body = Elements::new();
        let cond = stmt![type_var, " === ", string(name.as_ref())];
        body.push(js![if cond, js![return type_name, ".decode(", &data, ")"]]);
        body
    }

    fn raise_bad_type(&self, type_var: &Self::Stmt) -> Self::Stmt {
        js![throw string("bad type: "), " + ", type_var]
    }

    fn new_decode_method(&self, data: &Self::Stmt, body: Self::Elements) -> Self::Method {
        let mut decode = MethodSpec::with_static("decode");
        decode.push_argument(data.clone());
        decode.push(body);
        decode
    }
}

impl DynamicEncode for JsBackend {
    fn name_encode(&self, input: &Statement, _: Self::Type) -> Self::Stmt {
        stmt![input, ".encode()"]
    }

    fn array_encode(&self, input: &Statement, inner: Statement) -> Self::Stmt {
        stmt![input, ".map(function(v) { return ", inner, "; })"]
    }

    fn map_encode(&self, input: &Statement, _: Statement, value: Statement) -> Self::Stmt {
        let mut elements = Statement::new();

        elements.push("(function(data) {");
        elements.push(" let o = {};");
        elements.push(" for (let k in data) {");
        elements.push(stmt![" o[k] = ", &value, ";"]);
        elements.push(" };");
        elements.push(" return o;");
        elements.push(stmt![" })(", input, ")"]);

        elements
    }
}
