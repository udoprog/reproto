//! encoding/json module for Go

use backend::Initializer;
use compiler::Compiler;
use core;
use core::errors::{Error, Result};
use core::flavored::{RpEnumBody, RpInterfaceBody, RpSubType, RpTupleBody};
use genco::go::{imported, Go};
use genco::{Quoted, Tokens};
use std::rc::Rc;
use {EnumAdded, EnumCodegen, FieldAdded, FieldCodegen, InterfaceAdded, InterfaceCodegen, Options,
     TupleAdded, TupleCodegen};

pub struct Module {}

impl Module {
    pub fn new() -> Module {
        Module {}
    }
}

impl Initializer for Module {
    type Options = Options;

    fn initialize(&self, options: &mut Self::Options) -> Result<()> {
        let codegen = Rc::new(Codegen::new());
        options.field_gens.push(Box::new(codegen.clone()));
        options.enum_gens.push(Box::new(codegen.clone()));
        options.tuple_gens.push(Box::new(codegen.clone()));
        options.interface_gens.push(Box::new(codegen.clone()));
        Ok(())
    }
}

struct Codegen {
    new_error: Go<'static>,
    unmarshal: Go<'static>,
    marshal: Go<'static>,
    raw_message: Go<'static>,
}

impl Codegen {
    pub fn new() -> Codegen {
        Self {
            new_error: imported("errors", "New"),
            unmarshal: imported("encoding/json", "Unmarshal"),
            marshal: imported("encoding/json", "Marshal"),
            raw_message: imported("encoding/json", "RawMessage"),
        }
    }
}

impl FieldCodegen for Codegen {
    fn generate(&self, e: FieldAdded) -> Result<()> {
        let FieldAdded { tags, field, .. } = e;

        tags.push_str("json", field.name());

        if field.is_optional() {
            tags.push_str("json", "omitempty");
        }

        return Ok(());
    }
}

impl EnumCodegen for Codegen {
    fn generate(&self, e: EnumAdded) -> Result<()> {
        let EnumAdded {
            container,
            name,
            body,
            ..
        } = e;

        container.push(unmarshal_json(self, name.clone(), body));
        container.push(marshal_json(self, name.clone(), body));

        return Ok(());

        fn unmarshal_json<'el>(
            c: &Codegen,
            name: Go<'el>,
            body: &'el RpEnumBody,
        ) -> Tokens<'el, Go<'el>> {
            let mut t = Tokens::new();

            push!(t, "func (this *", name, ") UnmarshalJSON(b []byte) error {");

            t.nested({
                let mut t = Tokens::new();

                t.push("var s string");

                t.push_into(|t| {
                    push!(t, "if err := ", c.unmarshal, "(b, &s); err != nil {");
                    nested!(t, "return err");
                    push!(t, "}");
                });

                t.push_into(|t| {
                    t.push("switch s {");

                    for v in &body.variants {
                        t.push_into(|t| {
                            push!(t, "case ", v.ordinal().quoted(), ":");
                            nested!(t, "*this = ", name, "_", v.ident.as_str());
                        });
                    }

                    t.push_into(|t| {
                        push!(t, "default:");
                        nested!(t, "return ", c.new_error, "(", "bad value".quoted(), ")");
                    });

                    t.push("}");
                });

                t.push("return nil");

                t.join_line_spacing()
            });

            t.push("}");

            t
        }

        fn marshal_json<'el>(
            c: &Codegen,
            name: Go<'el>,
            body: &'el RpEnumBody,
        ) -> Tokens<'el, Go<'el>> {
            let mut t = Tokens::new();

            push!(t, "func (this ", name, ") MarshalJSON() ([]byte, error) {");

            t.nested({
                let mut t = Tokens::new();

                t.push("var s string");

                t.push_into(|t| {
                    t.push("switch this {");

                    for v in &body.variants {
                        t.push_into(|t| {
                            t.push(toks!["case ", name.clone(), "_", v.ident.as_str(), ":"]);
                            t.nested(toks!["s = ", v.ordinal().quoted()]);
                        });
                    }

                    t.push_into(|t| {
                        push!(t, "default:");
                        nested!(
                            t,
                            "return nil, ",
                            c.new_error,
                            "(",
                            "bad value".quoted(),
                            ")"
                        );
                    });

                    t.push("}");
                });

                push!(t, "return ", c.marshal, "(s)");

                t.join_line_spacing()
            });

            t.push("}");

            t
        }
    }
}

impl TupleCodegen for Codegen {
    fn generate(&self, e: TupleAdded) -> Result<()> {
        let TupleAdded {
            container,
            name,
            body,
            compiler,
            ..
        } = e;

        container.push(unmarshal_json(self, name.clone(), body, compiler)?);
        container.push(marshal_json(self, name.clone(), body)?);

        return Ok(());

        fn unmarshal_json<'el>(
            c: &Codegen,
            name: Go<'el>,
            body: &'el RpTupleBody,
            compiler: &Compiler<'el>,
        ) -> Result<Tokens<'el, Go<'el>>> {
            let mut t = Tokens::new();

            t.try_push_into::<Error, _>(|t| {
                push!(
                    t,
                    "func (this *",
                    name.clone(),
                    ") UnmarshalJSON(b []byte) error {"
                );

                t.nested({
                    let mut t = Tokens::new();

                    t.push(toks!["var array []", c.raw_message.clone()]);

                    t.push_into(|t| {
                        t.push(toks![
                            "if err := ",
                            c.unmarshal.clone(),
                            "(b, &array); err != nil {"
                        ]);
                        t.nested("return err");
                        t.push("}");
                    });

                    t.push({
                        let mut t = Tokens::new();

                        for (i, f) in body.fields.iter().enumerate() {
                            let a = toks!["array[", i.to_string(), "]"];

                            let ty = compiler.field_type(&f.ty)?;
                            let var = f.safe_ident();

                            t.push({
                                let mut t = Tokens::new();

                                t.push_into(|t| {
                                    t.push(toks!["var ", var.clone(), " ", ty]);

                                    t.push_into(|t| {
                                        t.push(toks![
                                            "if err := ",
                                            c.unmarshal.clone(),
                                            "(",
                                            a,
                                            ", &",
                                            var.clone(),
                                            "); err != nil {"
                                        ]);
                                        t.nested("return err");
                                        t.push("}");
                                    });

                                    t.push(toks!["this.", f.safe_ident(), " = ", var.clone()]);
                                });

                                t.join_line_spacing()
                            });
                        }

                        t.join_line_spacing()
                    });

                    t.push("return nil");

                    t.join_line_spacing()
                });

                t.push("}");

                Ok(())
            })?;

            Ok(t)
        }

        fn marshal_json<'el>(
            c: &Codegen,
            name: Go<'el>,
            body: &'el RpTupleBody,
        ) -> Result<Tokens<'el, Go<'el>>> {
            let mut t = Tokens::new();

            push!(t, "func (this ", name, ") MarshalJSON() ([]byte, error) {");

            t.nested({
                let mut t = Tokens::new();

                push!(t, "var array []", c.raw_message);

                t.push({
                    let mut t = Tokens::new();

                    for f in &body.fields {
                        let var = f.safe_ident();

                        t.push({
                            let mut t = Tokens::new();
                            let ident = toks!["this.", f.safe_ident()];

                            push!(t, var, ", err := ", c.marshal, "(", ident, ")");

                            t.push_into(|t| {
                                t.push("if err != nil {");
                                t.nested("return nil, err");
                                t.push("}");
                            });

                            push!(t, "array = append(array, ", var, ")");

                            t.join_line_spacing()
                        });
                    }

                    t.join_line_spacing()
                });

                push!(t, "return ", c.marshal, "(array)");

                t.join_line_spacing()
            });

            t.push("}");

            Ok(t)
        }
    }
}

impl InterfaceCodegen for Codegen {
    fn generate(&self, e: InterfaceAdded) -> Result<()> {
        let InterfaceAdded {
            container,
            name,
            body,
            compiler,
            ..
        } = e;

        container.push(unmarshal_json(self, name.clone(), body, compiler)?);
        container.push(marshal_json(self, name.clone(), body)?);

        return Ok(());

        fn unmarshal_json<'el>(
            c: &Codegen,
            name: Go<'el>,
            body: &'el RpInterfaceBody,
            compiler: &Compiler<'el>,
        ) -> Result<Tokens<'el, Go<'el>>> {
            let mut t = Tokens::new();

            t.try_push_into::<Error, _>(|t| {
                push!(t, "func (this *", name, ") UnmarshalJSON(b []byte) error {");

                match body.sub_type_strategy {
                    core::RpSubTypeStrategy::Tagged { ref tag } => {
                        t.nested(unmarshal_envelope(c, body, compiler, tag)?);
                    }
                }

                push!(t, "}");
                Ok(())
            })?;

            return Ok(t);

            /// Unmarshal the envelope and extract the type field.
            fn unmarshal_envelope<'el>(
                c: &Codegen,
                body: &'el RpInterfaceBody,
                compiler: &Compiler<'el>,
                tag: &'el str,
            ) -> Result<Tokens<'el, Go<'el>>> {
                let mut t = Tokens::new();

                t.push_into(|t| {
                    push!(t, "var err error");
                    push!(t, "var ok bool");
                    push!(t, "env := make(map[string]", c.raw_message, ")");
                });

                t.push_into(|t| {
                    push!(t, "if err := ", c.unmarshal, "(b, &env); err != nil {");
                    nested!(t, "return err");
                    push!(t, "}");
                });

                push!(t, "var raw_tag ", c.raw_message);

                t.push_into(|t| {
                    push!(t, "if raw_tag, ok = env[", tag.quoted(), "]; !ok {");
                    nested!(t, "return ", c.new_error, "(", "missing tag".quoted(), ")");
                    push!(t, "}");
                });

                push!(t, "var tag string");

                t.push_into(|t| {
                    push!(t, "if err = ", c.unmarshal, "(raw_tag, &tag); err != nil {");
                    nested!(t, "return err");
                    push!(t, "}");
                });

                t.try_push_into::<Error, _>(|t| {
                    push!(t, "switch (tag) {");

                    for sub_type in &body.sub_types {
                        let name = compiler.convert_name(&sub_type.name)?;
                        push!(t, "case ", sub_type.name().quoted(), ":");

                        t.nested({
                            let mut t = Tokens::new();

                            push!(t, "sub := ", name, "{}");

                            t.push_into(|t| {
                                push!(t, "if err = ", c.unmarshal, "(b, &sub); err != nil {");
                                nested!(t, "return err");
                                push!(t, "}");
                            });

                            t.push_into(|t| {
                                push!(t, "this.", sub_type.ident, " = &sub");
                                push!(t, "return nil");
                            });

                            t.join_line_spacing()
                        });
                    }

                    push!(t, "default:");
                    nested!(t, "return ", c.new_error, "(", "bad tag".quoted(), ")");

                    push!(t, "}");
                    Ok(())
                })?;

                Ok(t.join_line_spacing())
            }
        }

        fn marshal_json<'el>(
            c: &Codegen,
            name: Go<'el>,
            body: &'el RpInterfaceBody,
        ) -> Result<Tokens<'el, Go<'el>>> {
            let mut t = Tokens::new();

            t.push({
                let mut t = Tokens::new();

                push!(t, "func (this ", name, ") MarshalJSON() ([]byte, error) {");

                match body.sub_type_strategy {
                    core::RpSubTypeStrategy::Tagged { ref tag } => {
                        t.nested(marshal_envelope(c, body, tag)?);
                    }
                }

                push!(t, "}");

                t
            });

            return Ok(t);

            /// Marshal the envelope and extract the type field.
            fn marshal_envelope<'el>(
                c: &Codegen,
                body: &'el RpInterfaceBody,
                tag: &'el str,
            ) -> Result<Tokens<'el, Go<'el>>> {
                let mut t = Tokens::new();

                t.push_into(|t| {
                    push!(t, "var b []byte");
                    push!(t, "var err error");
                    push!(t, "env := make(map[string]", c.raw_message, ")");
                });

                t.push({
                    let mut t = Tokens::new();

                    for sub_type in &body.sub_types {
                        let ident = toks!("this.", sub_type.ident.clone());
                        t.push(sub_type_check(c, ident, sub_type, tag));
                    }

                    t.join_line_spacing()
                });

                let error = toks!(c.new_error.clone(), "(", "no sub-type set".quoted(), ")");
                push!(t, "return nil, ", error);

                return Ok(t.join_line_spacing());
            }

            fn sub_type_check<'el>(
                c: &Codegen,
                ident: Tokens<'el, Go<'el>>,
                sub_type: &'el RpSubType,
                tag: &'el str,
            ) -> Tokens<'el, Go<'el>> {
                let mut t = Tokens::new();

                push!(t, "if this.", sub_type.ident, " != nil {");

                t.nested({
                    let mut t = Tokens::new();

                    t.push_into(|t| {
                        push!(t, "if b, err = ", c.marshal, "(&", ident, "); err != nil {");
                        nested!(t, "return nil, err");
                        push!(t, "}");
                    });

                    t.push_into(|t| {
                        push!(t, "if err = ", c.unmarshal, "(b, &env); err != nil {");
                        nested!(t, "return nil, err");
                        push!(t, "}");
                    });

                    let o = toks!("env[", tag.quoted(), "]");

                    t.push_into(|t| {
                        let m = toks!(c.marshal.clone(), "(", sub_type.name().quoted(), ")");

                        push!(t, "if ", o, ", err = ", m, "; err != nil {");
                        nested!(t, "return nil, err");
                        push!(t, "}");
                    });

                    push!(t, "return ", c.marshal, "(env)");

                    t.join_line_spacing()
                });

                push!(t, "}");

                t
            }
        }
    }
}
