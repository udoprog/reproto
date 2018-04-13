use backend::PackageProcessor;
use core::errors::*;
use core::{self, Handle, Loc};
use flavored::{JavaScriptFlavor, JavaScriptName, RpEnumBody, RpField, RpInterfaceBody,
               RpTupleBody, RpTypeBody};
use genco::{Element, JavaScript, Quoted, Tokens};
use naming::{self, Naming};
use std::rc::Rc;
use trans::{self, Translated};
use utils::{is_defined, is_not_defined};
use {FileSpec, Options, EXT};

pub struct Compiler<'el> {
    pub env: &'el Translated<JavaScriptFlavor>,
    variant_field: &'el Loc<RpField>,
    handle: &'el Handle,
    to_lower_snake: naming::ToLowerSnake,
    values: Tokens<'static, JavaScript<'static>>,
    enum_name: Tokens<'static, JavaScript<'static>>,
}

impl<'el> Compiler<'el> {
    pub fn new(
        env: &'el Translated<JavaScriptFlavor>,
        variant_field: &'el Loc<RpField>,
        _: Options,
        handle: &'el Handle,
    ) -> Compiler<'el> {
        Compiler {
            env,
            variant_field,
            handle,
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
            let value_toks = field.ty.encode(field_toks.clone());

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
            values.push(field.ty.encode(toks));
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
        name: &'el JavaScriptName,
        ident: &'el str,
    ) -> Result<Tokens<'el, JavaScript<'el>>> {
        let members = toks![name, ".", self.values.clone()];
        let loop_init = toks!["let i = 0, l = ", members.clone(), ".length"];
        let match_member = toks!["member.", ident, " === data"];

        let mut loop_body = Tokens::new();
        loop_body.push(toks!["const member = ", members, "[i]"]);
        loop_body.push(js![if match_member, toks!["return member;"]]);

        let mut member_loop = Tokens::new();
        member_loop.push(js![for loop_init; "i < l"; "i++", loop_body.join_line_spacing()]);

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
        name: &'el JavaScriptName,
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
                let var_toks = field.ty.decode(var_name.clone());

                let mut check = Tokens::new();

                check.push(toks!["let ", var_name.clone(), " = data[", var, "];"]);
                check.push(js![if is_defined(var_name.clone()),
                                      toks![var_name.clone(), " = ", var_toks, ";"],
                                      toks![var_name, " = null", ";"]]);

                check.join_line_spacing()
            } else {
                let var_toks = toks!["data[", var.clone(), "]"];
                let var_toks = field.ty.decode(var_toks.into());

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

        body.push(js![@return new name, arguments]);

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
        name: &'el JavaScriptName,
    ) -> Result<Tokens<'el, JavaScript<'el>>> {
        let mut elements = Tokens::new();

        elements.push({
            let mut encode = Tokens::new();
            encode.push("encode() {");
            encode.nested(js![return "this.", field.safe_ident()]);
            encode.push("}");
            encode
        });

        let decode = self.decode_enum_method(name, field.safe_ident())?;
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

impl<'el> PackageProcessor<'el, JavaScriptFlavor, JavaScriptName> for Compiler<'el> {
    type Out = FileSpec<'el>;
    type DeclIter = trans::translated::DeclIter<'el, JavaScriptFlavor>;

    fn ext(&self) -> &str {
        EXT
    }

    fn decl_iter(&self) -> Self::DeclIter {
        self.env.decl_iter()
    }

    fn handle(&self) -> &'el Handle {
        self.handle
    }

    fn process_tuple(&self, out: &mut Self::Out, body: &'el RpTupleBody) -> Result<()> {
        let mut class_body = Tokens::new();

        class_body.push(self.build_constructor(&body.fields));

        // TODO: make configurable
        if false {
            for getter in self.build_getters(&body.fields)? {
                class_body.push(getter);
            }
        }

        class_body.push(self.decode_method(&body.fields, &body.name, Self::field_by_index)?);

        class_body.push(self.encode_tuple_method(&body.fields)?);
        class_body.push_unless_empty(code!(&body.codes, core::RpContext::Js));

        let mut class = Tokens::new();

        class.push(toks!["export class ", &body.name, " {"]);
        class.nested(class_body.join_line_spacing());
        class.push("}");

        out.0.push(class);
        Ok(())
    }

    fn process_enum(&self, out: &mut Self::Out, body: &'el RpEnumBody) -> Result<()> {
        let mut class_body = Tokens::new();

        let mut members = Tokens::new();

        class_body.push(self.build_enum_constructor(self.variant_field));
        class_body.push(self.enum_encode_decode(self.variant_field, &body.name)?);

        let mut values = Tokens::new();

        for v in body.variants.iter() {
            let mut args = Tokens::new();

            args.append(v.ident().quoted());

            match v.value {
                core::RpVariantValue::String(string) => {
                    args.append(string.quoted());
                }
                core::RpVariantValue::Number(number) => {
                    args.append(number.to_string());
                }
            }

            let args = js![new & body.name, args];
            let member = toks![&body.name, ".", v.ident()];

            values.push(js![= member.clone(), args]);
            members.append(member);
        }

        class_body.push_unless_empty(code!(&body.codes, core::RpContext::Js));

        let mut elements = Tokens::new();

        let mut class = Tokens::new();

        class.push(toks!["export class ", &body.name, " {"]);
        class.nested(class_body.join_line_spacing());
        class.push("}");

        // class declaration
        elements.push(class);

        // enum literal values
        elements.push(values);

        // push members field
        let members_key = toks![&body.name, ".", self.values.clone()];
        elements.push(js![= members_key, js!([members])]);

        out.0.push(elements.join_line_spacing());
        Ok(())
    }

    fn process_type(&self, out: &mut Self::Out, body: &'el RpTypeBody) -> Result<()> {
        let mut class_body = Tokens::new();

        class_body.push(self.build_constructor(&body.fields));

        // TODO: make configurable
        if false {
            for getter in self.build_getters(&body.fields)? {
                class_body.push(getter);
            }
        }

        class_body.push(self.decode_method(&body.fields, &body.name, Self::field_by_name)?);

        class_body.push(self.encode_method(&body.fields, "{}", None)?);
        class_body.push_unless_empty(code!(&body.codes, core::RpContext::Js));

        let mut class = Tokens::new();

        class.push(toks!["export class ", &body.name, " {"]);
        class.nested(class_body.join_line_spacing());
        class.push("}");

        out.0.push(class);
        Ok(())
    }

    fn process_interface(&self, out: &mut Self::Out, body: &'el RpInterfaceBody) -> Result<()> {
        let mut classes = Tokens::new();

        let mut interface_body = Tokens::new();

        match body.sub_type_strategy {
            core::RpSubTypeStrategy::Tagged { ref tag, .. } => {
                let tk = tag.as_str().quoted().into();
                interface_body.push(decode(&body, &tk)?);
            }
            core::RpSubTypeStrategy::Untagged => {
                interface_body.push(decode_untagged(body)?);
            }
        }

        interface_body.push_unless_empty(code!(&body.codes, core::RpContext::Js));

        classes.push({
            let mut tokens = Tokens::new();

            tokens.push(toks!["export class ", &body.name, " {"]);
            tokens.nested(interface_body.join_line_spacing());
            tokens.push("}");

            tokens
        });

        for sub_type in &body.sub_types {
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
                &sub_type.name,
                Self::field_by_name,
            )?);

            match body.sub_type_strategy {
                core::RpSubTypeStrategy::Tagged { ref tag, .. } => {
                    let tk: Tokens<'el, JavaScript<'el>> = tag.as_str().quoted().into();
                    let type_toks = toks!["data[", tk, "] = ", sub_type.name().quoted(), ";"];
                    class_body.push(self.encode_method(
                        fields.iter().cloned(),
                        "{}",
                        Some(type_toks),
                    )?);
                }
                core::RpSubTypeStrategy::Untagged => {
                    class_body.push(self.encode_method(fields.iter().cloned(), "{}", None)?);
                }
            }

            class_body.push_unless_empty(code!(&sub_type.codes, core::RpContext::Js));

            classes.push({
                let mut tokens = Tokens::new();

                tokens.push(toks!["export class ", &sub_type.name, " {"]);
                tokens.nested(class_body.join_line_spacing());
                tokens.push("}");

                tokens
            });
        }

        out.0.push(classes.join_line_spacing());
        return Ok(());

        fn decode<'el>(
            body: &'el RpInterfaceBody,
            tag: &Tokens<'el, JavaScript<'el>>,
        ) -> Result<Tokens<'el, JavaScript<'el>>> {
            let mut t = Tokens::new();

            let data = "data";
            let f_tag = "f_tag";

            push!(t, "const ", f_tag, " = ", data, "[", tag.clone(), "]");

            for sub_type in body.sub_types.iter() {
                t.push_into(|t| {
                    let cond = toks![f_tag, " === ", sub_type.name().quoted()];
                    t.push(js![if cond, js![return &sub_type.name, ".decode(", data, ")"]]);
                });
            }

            t.push(js![throw "bad type: ".quoted(), " + ", f_tag]);

            Ok({
                let mut decode = Tokens::new();
                push!(decode, "static decode(", data, ") {");
                decode.nested(t.join_line_spacing());
                push!(decode, "}");
                decode
            })
        }

        fn decode_untagged<'el>(
            body: &'el RpInterfaceBody,
        ) -> Result<Tokens<'el, JavaScript<'el>>> {
            let mut t = Tokens::new();

            let data = "data";

            push!(t, "var all = true");
            push!(t, "var keys = {}");

            t.push_into(|t| {
                push!(t, "for (const k in ", data, ") {");
                nested!(t, "keys[k] = true");
                push!(t, "}");
            });

            for sub_type in body.sub_types.iter() {
                let mut required = Tokens::new();

                for f in sub_type.discriminating_fields() {
                    required.append(toks!["(", f.name().quoted(), " in keys)"]);
                }

                t.push_into(|t| {
                    let cond = required.join(" && ");
                    t.push(js![if cond, js![return &sub_type.name, ".decode(", data, ")"]]);
                });
            }

            t.push(js![throw "no legal field combinations found".quoted()]);

            Ok({
                let mut decode = Tokens::new();
                push!(decode, "static decode(", data, ") {");
                decode.nested(t.join_line_spacing());
                push!(decode, "}");
                decode
            })
        }
    }
}
