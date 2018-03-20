use {EXT, FileSpec, Options, TYPE_SEP};
use backend::{Converter, DynamicConverter, DynamicDecode, DynamicEncode, PackageProcessor,
              PackageUtils};
use core::{ForEachLoc, Handle, Loc, RpContext, RpEnumBody, RpField, RpInterfaceBody, RpName,
           RpPackage, RpSubTypeStrategy, RpTupleBody, RpType, RpTypeBody, RpVersionedPackage};
use core::errors::*;
use genco::{Element, JavaScript, Quoted, Tokens};
use genco::js::imported_alias;
use naming::{self, Naming};
use std::rc::Rc;
use trans::{self, Environment};
use utils::{is_defined, is_not_defined};

pub struct Compiler<'el> {
    pub env: &'el Environment,
    variant_field: &'el Loc<RpField>,
    handle: &'el Handle,
    to_lower_snake: naming::ToLowerSnake,
    values: Tokens<'static, JavaScript<'static>>,
    enum_name: Tokens<'static, JavaScript<'static>>,
}

impl<'el> Compiler<'el> {
    pub fn new(
        env: &'el Environment,
        variant_field: &'el Loc<RpField>,
        _: Options,
        handle: &'el Handle,
    ) -> Compiler<'el> {
        Compiler {
            env: env,
            variant_field: variant_field,
            handle: handle,
            to_lower_snake: naming::to_lower_snake(),
            values: "values".into(),
            enum_name: "name".into(),
        }
    }

    pub fn compile(&self) -> Result<()> {
        let files = self.populate_files()?;
        self.write_files(files)
    }

    /// Build a function that throws an exception if the given value `toks` is None.
    fn throw_if_null<S>(&self, toks: S, field: &Loc<RpField>) -> Tokens<'el, JavaScript<'el>>
    where
        S: Into<Tokens<'el, JavaScript<'el>>>,
    {
        let required_error = format!("{}: is a required field", field.name()).quoted();
        js![if is_not_defined(toks), js![throw required_error]]
    }

    fn encode_method<B, I>(
        &self,
        fields: I,
        builder: B,
        extra: Option<Tokens<'el, JavaScript<'el>>>,
    ) -> Result<Tokens<'el, JavaScript<'el>>>
    where
        B: Into<Tokens<'el, JavaScript<'el>>>,
        I: IntoIterator<Item = &'el Loc<RpField>>,
    {
        let mut body = Tokens::new();

        body.push(toks!["const data = ", builder.into(), ";"]);

        if let Some(extra) = extra {
            body.push(extra);
        }

        let mut assign = Tokens::new();

        for field in fields {
            let var_string = field.name().quoted();
            let field_toks = toks!["this.", field.safe_ident()];
            let value_toks = self.dynamic_encode(&field.ty, field_toks.clone())?;

            if field.is_optional() {
                let toks = js![if is_defined(field_toks),
                                      toks!["data[", var_string, "] = ", value_toks, ";"]];
                assign.push(toks);
            } else {
                assign.push(self.throw_if_null(field_toks, field));
                let toks = toks!["data[", var_string, "] = ", value_toks, ";"];
                assign.push(toks);
            }
        }

        if !assign.is_empty() {
            body.push(assign.join_line_spacing());
        }

        body.push(js![return "data"]);

        Ok({
            let mut t = Tokens::new();
            t.push("encode() {");
            t.nested(body.join_line_spacing());
            t.push("}");
            t
        })
    }

    fn encode_tuple_method<I>(&self, fields: I) -> Result<Tokens<'el, JavaScript<'el>>>
    where
        I: IntoIterator<Item = &'el Loc<RpField>>,
    {
        let mut values = Tokens::new();

        let mut body = Tokens::new();

        for field in fields {
            let toks = toks!["this.", field.safe_ident()];
            body.push(self.throw_if_null(toks.clone(), field));
            values.push(self.dynamic_encode(&field.ty, toks)?);
        }

        body.push(js![@return [ values ]]);

        let mut encode = Tokens::new();
        encode.push("encode() {");
        encode.nested(body.join_line_spacing());
        encode.push("}");
        Ok(encode)
    }

    fn decode_enum_method(
        &self,
        type_name: Rc<String>,
        ident: &'el str,
    ) -> Result<Tokens<'el, JavaScript<'el>>> {
        let members = toks![type_name, ".", self.values.clone()];
        let loop_init = toks!["let i = 0, l = ", members.clone(), ".length"];
        let match_member = toks!["member.", ident, " === data"];

        let mut loop_body = Tokens::new();
        loop_body.push(toks!["const member = ", members, "[i]"]);
        loop_body.push(js![if match_member, toks!["return member;"]]);

        let mut member_loop = Tokens::new();
        member_loop.push(
            js![for loop_init; "i < l"; "i++", loop_body.join_line_spacing()],
        );

        let mut body = Tokens::new();
        body.push(member_loop);
        body.push(js![throw "no matching value: ".quoted(), " + data"]);

        let mut decode = Tokens::new();
        decode.push("static decode(data) {");
        decode.nested(body.join_line_spacing());
        decode.push("}");
        Ok(decode)
    }

    fn decode_method<F, I>(
        &self,
        fields: I,
        type_name: Rc<String>,
        variable_fn: F,
    ) -> Result<Tokens<'el, JavaScript<'el>>>
    where
        F: Fn(usize, &'el Loc<RpField>) -> Element<'el, JavaScript<'el>>,
        I: IntoIterator<Item = &'el Loc<RpField>>,
    {
        let mut arguments = Tokens::new();
        let mut assign = Tokens::new();

        for (i, field) in fields.into_iter().enumerate() {
            let var_name = Rc::new(format!("v_{}", field.ident()));
            let var = variable_fn(i, field);

            let toks = if field.is_optional() {
                let var_name = toks![var_name.clone()];
                let var_toks = self.dynamic_decode(&field.ty, var_name.clone())?;

                let mut check = Tokens::new();

                check.push(toks!["let ", var_name.clone(), " = data[", var, "];"]);
                check.push(js![if is_defined(var_name.clone()),
                                      toks![var_name.clone(), " = ", var_toks, ";"],
                                      toks![var_name, " = null", ";"]]);

                check.join_line_spacing()
            } else {
                let var_toks = toks!["data[", var.clone(), "]"];
                let var_toks = self.dynamic_decode(&field.ty, var_toks.into())?;

                let mut check = Tokens::new();

                let var_name = toks![var_name.clone()];

                check.push(toks!["const ", var_name.clone(), " = ", var_toks, ";"]);
                check.push(js![if is_not_defined(var_name),
                                   js![throw var, " + ", ": required field".quoted()]]);

                check.join_line_spacing()
            };

            assign.push(toks);
            arguments.append(var_name);
        }

        let mut body = Tokens::new();

        if !assign.is_empty() {
            body.push(assign.join_line_spacing());
        }

        body.push(js![@return new type_name, arguments]);

        let mut decode = Tokens::new();
        decode.push("static decode(data) {");
        decode.nested(body.join_line_spacing());
        decode.push("}");
        Ok(decode)
    }

    fn field_by_name(_i: usize, field: &'el Loc<RpField>) -> Element<'el, JavaScript<'el>> {
        field.name().quoted()
    }

    fn field_by_index(i: usize, _field: &'el Loc<RpField>) -> Element<'el, JavaScript<'el>> {
        i.to_string().into()
    }

    fn build_constructor<I>(&self, fields: I) -> Tokens<'el, JavaScript<'el>>
    where
        I: IntoIterator<Item = &'el Loc<RpField>>,
    {
        let mut arguments = Tokens::new();
        let mut assignments = Tokens::new();

        for field in fields {
            arguments.append(field.safe_ident());
            assignments.push(toks![
                "this.",
                field.safe_ident(),
                " = ",
                field.safe_ident(),
                ";",
            ]);
        }

        let mut ctor = Tokens::new();
        ctor.push(toks!["constructor(", arguments.join(", "), ") {"]);
        ctor.nested(assignments);
        ctor.push("}");
        ctor
    }

    fn build_enum_constructor<'a>(&self, field: &'el RpField) -> Tokens<'el, JavaScript<'el>> {
        let mut arguments = Tokens::new();
        let mut assignments = Tokens::new();

        arguments.append(self.enum_name.clone());
        assignments.push(toks![
            "this.",
            self.enum_name.clone(),
            " = ",
            self.enum_name.clone(),
            ";",
        ]);

        arguments.append(field.safe_ident());
        assignments.push(toks![
            "this.",
            field.safe_ident(),
            " = ",
            field.safe_ident(),
            ";",
        ]);

        let mut ctor = Tokens::new();
        ctor.push(toks!["constructor(", arguments.join(", "), ") {"]);
        ctor.nested(assignments);
        ctor.push("}");
        ctor
    }

    fn enum_encode_decode(
        &self,
        field: &'el Loc<RpField>,
        type_name: Rc<String>,
    ) -> Result<Tokens<'el, JavaScript<'el>>> {
        let mut elements = Tokens::new();

        elements.push({
            let mut encode = Tokens::new();
            encode.push("encode() {");
            encode.nested(js![return "this.", field.safe_ident()]);
            encode.push("}");
            encode
        });

        let decode = self.decode_enum_method(type_name, field.safe_ident())?;
        elements.push(decode);
        return Ok(elements.into());
    }

    fn build_getters<I>(&self, fields: I) -> Result<Vec<Tokens<'el, JavaScript<'el>>>>
    where
        I: IntoIterator<Item = &'el Loc<RpField>>,
    {
        let mut result = Vec::new();

        for field in fields {
            let name = Rc::new(self.to_lower_snake.convert(&field.ident));

            result.push({
                let mut tokens = Tokens::new();
                tokens.push(toks!["function get_", name.clone(), "() {"]);
                tokens.push(js![return "this.", name]);
                tokens.push("}");
                tokens
            });
        }

        Ok(result)
    }
}

impl<'el> PackageUtils for Compiler<'el> {}

impl<'el> Converter<'el> for Compiler<'el> {
    type Custom = JavaScript<'el>;

    fn convert_type(&self, name: &RpName) -> Result<Tokens<'el, JavaScript<'el>>> {
        let registered = self.env.lookup(name)?;

        let ident = registered.ident(name, |p| p.join(TYPE_SEP), |c| c.join(TYPE_SEP));

        if let Some(ref used) = name.prefix {
            let package = self.package(&name.package).parts.join(".");
            return Ok(imported_alias(package, ident, used.to_string()).into());
        }

        Ok(ident.into())
    }
}

impl<'el> DynamicConverter<'el> for Compiler<'el> {
    fn is_native(&self, ty: &RpType) -> bool {
        use self::RpType::*;

        match *ty {
            Signed { size: _ } |
            Unsigned { size: _ } => true,
            Float | RpType::Double => true,
            String => true,
            Any => true,
            Boolean => true,
            Array { ref inner } => self.is_native(inner),
            Map { ref key, ref value } => self.is_native(key) && self.is_native(value),
            _ => false,
        }
    }

    fn map_key_var(&self) -> Tokens<'el, JavaScript<'el>> {
        toks!["k"]
    }

    fn map_value_var(&self) -> Tokens<'el, JavaScript<'el>> {
        toks!["data[k]"]
    }

    fn array_inner_var(&self) -> Tokens<'el, JavaScript<'el>> {
        toks!["v"]
    }
}

impl<'el> DynamicDecode<'el> for Compiler<'el> {
    fn name_decode(
        &self,
        input: Tokens<'el, JavaScript<'el>>,
        name: Tokens<'el, JavaScript<'el>>,
    ) -> Tokens<'el, JavaScript<'el>> {
        toks![name, ".decode(", input, ")"]
    }

    /// Decoding an Array in JavaScript.
    ///
    /// Maps over each decoded value using `Array.map(...)`, decoding each variable.
    fn array_decode(
        &self,
        input: Tokens<'el, JavaScript<'el>>,
        inner: Tokens<'el, JavaScript<'el>>,
    ) -> Tokens<'el, JavaScript<'el>> {
        toks![input, ".map(function(v) { return ", inner, "; })"]
    }

    /// Decoding a map in JavaScript.
    fn map_decode(
        &self,
        input: Tokens<'el, JavaScript<'el>>,
        _key: Tokens<'el, JavaScript<'el>>,
        value: Tokens<'el, JavaScript<'el>>,
    ) -> Tokens<'el, JavaScript<'el>> {
        let mut t = Tokens::new();

        t.append("(function(data) {");
        t.append(" let o = {};");
        t.append(" for (let k in data) {");
        t.append(toks![" o[k] = ", value, ";"]);
        t.append(" };");
        t.append(" return o;");
        t.append(toks![" })(", input, ")"]);

        t
    }

    fn assign_tag_var(
        &self,
        data: &'el str,
        tag_var: &'el str,
        tag: &Tokens<'el, JavaScript<'el>>,
    ) -> Tokens<'el, JavaScript<'el>> {
        toks!["const ", tag_var, " = ", data, "[", tag.clone(), "]",]
    }

    fn check_tag_var(
        &self,
        data: &'el str,
        tag_var: &'el str,
        name: &'el str,
        type_name: Tokens<'el, Self::Custom>,
    ) -> Tokens<'el, JavaScript<'el>> {
        let mut body = Tokens::new();
        let cond = toks![tag_var, " === ", name.quoted()];
        body.push(js![if cond, js![return type_name, ".decode(", data, ")"]]);
        body
    }

    fn raise_bad_type(&self, tag_var: &'el str) -> Tokens<'el, JavaScript<'el>> {
        js![throw "bad type: ".quoted(), " + ", tag_var]
    }

    fn new_decode_method(
        &self,
        data: &'el str,
        body: Tokens<'el, Self::Custom>,
    ) -> Tokens<'el, JavaScript<'el>> {
        let mut decode = Tokens::new();
        decode.push(toks!["static decode(", data, ") {"]);
        decode.nested(body);
        decode.push("}");
        decode
    }
}

impl<'el> DynamicEncode<'el> for Compiler<'el> {
    fn name_encode(
        &self,
        input: Tokens<'el, JavaScript<'el>>,
        _: Tokens<'el, JavaScript<'el>>,
    ) -> Tokens<'el, JavaScript<'el>> {
        toks![input, ".encode()"]
    }

    fn array_encode(
        &self,
        input: Tokens<'el, JavaScript<'el>>,
        inner: Tokens<'el, JavaScript<'el>>,
    ) -> Tokens<'el, JavaScript<'el>> {
        toks![input, ".map(function(v) { return ", inner, "; })"]
    }

    fn map_encode(
        &self,
        input: Tokens<'el, JavaScript<'el>>,
        _: Tokens<'el, JavaScript<'el>>,
        value: Tokens<'el, JavaScript<'el>>,
    ) -> Tokens<'el, JavaScript<'el>> {
        let mut t = Tokens::new();

        t.append("(function(data) {");
        t.append(" let o = {};");
        t.append(" for (let k in data) {");
        t.append(toks![" o[k] = ", value, ";"]);
        t.append(" };");
        t.append(" return o;");
        t.append(toks![" })(", input, ")"]);

        t
    }
}

impl<'el> PackageProcessor<'el> for Compiler<'el> {
    type Out = FileSpec<'el>;
    type DeclIter = trans::environment::DeclIter<'el>;

    fn ext(&self) -> &str {
        EXT
    }

    fn decl_iter(&self) -> Self::DeclIter {
        self.env.decl_iter()
    }

    fn handle(&self) -> &'el Handle {
        self.handle
    }

    fn processed_package(&self, package: &RpVersionedPackage) -> RpPackage {
        self.package(package)
    }

    fn process_tuple(&self, out: &mut Self::Out, body: &'el RpTupleBody) -> Result<()> {
        let tuple_name = Rc::new(body.name.join(TYPE_SEP));
        let mut class_body = Tokens::new();

        class_body.push(self.build_constructor(&body.fields));

        // TODO: make configurable
        if false {
            for getter in self.build_getters(&body.fields)? {
                class_body.push(getter);
            }
        }

        class_body.push(self.decode_method(
            &body.fields,
            tuple_name.clone(),
            Self::field_by_index,
        )?);

        class_body.push(self.encode_tuple_method(&body.fields)?);
        class_body.push_unless_empty(code!(&body.codes, RpContext::Js));

        let mut class = Tokens::new();

        class.push(toks!["export class ", tuple_name, " {"]);
        class.nested(class_body.join_line_spacing());
        class.push("}");

        out.0.push(class);
        Ok(())
    }

    fn process_enum(&self, out: &mut Self::Out, body: &'el RpEnumBody) -> Result<()> {
        let type_name = Rc::new(body.name.join(TYPE_SEP));

        let mut class_body = Tokens::new();

        let mut members = Tokens::new();

        class_body.push(self.build_enum_constructor(self.variant_field));
        class_body.push(self.enum_encode_decode(
            self.variant_field,
            type_name.clone(),
        )?);

        let mut values = Tokens::new();

        body.variants.iter().for_each_loc(|variant| {
            let type_id = self.convert_type(&body.name)?;
            let mut arguments = Tokens::new();

            arguments.append(variant.ident().quoted());
            arguments.append(variant.ordinal().quoted());

            let arguments = js![new type_id, arguments];
            let member = toks![type_name.clone(), ".", variant.ident()];

            values.push(js![= member.clone(), arguments]);
            members.append(member);

            Ok(()) as Result<()>
        })?;

        class_body.push_unless_empty(code!(&body.codes, RpContext::Js));

        let mut elements = Tokens::new();

        let mut class = Tokens::new();

        class.push(toks!["export class ", type_name.clone(), " {"]);
        class.nested(class_body.join_line_spacing());
        class.push("}");

        // class declaration
        elements.push(class);

        // enum literal values
        elements.push(values);

        // push members field
        let members_key = toks![type_name.clone(), ".", self.values.clone()];
        elements.push(js![= members_key, js!([members])]);

        out.0.push(elements.join_line_spacing());
        Ok(())
    }

    fn process_type(&self, out: &mut Self::Out, body: &'el RpTypeBody) -> Result<()> {
        let type_name = Rc::new(body.name.join(TYPE_SEP));

        let mut class_body = Tokens::new();

        class_body.push(self.build_constructor(&body.fields));

        // TODO: make configurable
        if false {
            for getter in self.build_getters(&body.fields)? {
                class_body.push(getter);
            }
        }

        class_body.push(self.decode_method(
            &body.fields,
            type_name.clone(),
            Self::field_by_name,
        )?);

        class_body.push(self.encode_method(&body.fields, "{}", None)?);
        class_body.push_unless_empty(code!(&body.codes, RpContext::Js));

        let mut class = Tokens::new();

        class.push(toks!["export class ", type_name, " {"]);
        class.nested(class_body.join_line_spacing());
        class.push("}");

        out.0.push(class);
        Ok(())
    }

    fn process_interface(&self, out: &mut Self::Out, body: &'el RpInterfaceBody) -> Result<()> {
        let mut classes = Tokens::new();
        let interface_type_name = Rc::new(body.name.join(TYPE_SEP));

        let mut interface_body = Tokens::new();

        match body.sub_type_strategy {
            RpSubTypeStrategy::Tagged { ref tag, .. } => {
                let tk = tag.as_str().quoted().into();
                interface_body.push(self.interface_decode_method(&body, &tk)?);
            }
        }

        interface_body.push_unless_empty(code!(&body.codes, RpContext::Js));

        classes.push({
            let mut tokens = Tokens::new();

            tokens.push(toks!["export class ", interface_type_name.clone(), " {"]);
            tokens.nested(interface_body.join_line_spacing());
            tokens.push("}");

            tokens
        });

        let sub_types = body.sub_types.iter().map(|t| Loc::as_ref(t));

        sub_types.for_each_loc(|sub_type| {
            let type_name = Rc::new(sub_type.name.join(TYPE_SEP));

            let mut class_body = Tokens::new();

            let fields: Vec<&Loc<RpField>> =
                body.fields.iter().chain(sub_type.fields.iter()).collect();

            class_body.push(self.build_constructor(fields.iter().cloned()));

            // TODO: make configurable
            if false {
                for getter in self.build_getters(fields.iter().cloned())? {
                    class_body.push(getter);
                }
            }

            class_body.push(self.decode_method(
                fields.iter().cloned(),
                type_name.clone(),
                Self::field_by_name,
            )?);

            match body.sub_type_strategy {
                RpSubTypeStrategy::Tagged { ref tag, .. } => {
                    let tk: Tokens<'el, JavaScript<'el>> = tag.as_str().quoted().into();
                    let type_toks = toks!["data[", tk, "] = ", sub_type.name().quoted(), ";"];
                    class_body.push(self.encode_method(
                        fields.iter().cloned(),
                        "{}",
                        Some(type_toks),
                    )?);
                }
            }

            class_body.push_unless_empty(code!(&sub_type.codes, RpContext::Js));

            classes.push({
                let mut tokens = Tokens::new();

                tokens.push(toks!["export class ", type_name.clone(), " {"]);
                tokens.nested(class_body.join_line_spacing());
                tokens.push("}");

                tokens
            });

            Ok(()) as Result<()>
        })?;

        out.0.push(classes.join_line_spacing());
        Ok(())
    }
}
