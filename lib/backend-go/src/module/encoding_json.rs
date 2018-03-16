//! encoding/json module for Go

use {EnumAdded, EnumCodegen, FieldAdded, FieldCodegen, InterfaceAdded, InterfaceCodegen, Options,
     TupleAdded, TupleCodegen};
use backend::Initializer;
use core::errors::Result;
use core::{RpEnumBody, RpInterfaceBody, RpTupleBody};
use std::rc::Rc;
use genco::{Quoted, Tokens};
use go::{imported, Go};
use compiler::Compiler;

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
            codegen: &Codegen,
            name: Go<'el>,
            body: &'el RpEnumBody,
        ) -> Tokens<'el, Go<'el>> {
            let mut t = Tokens::new();

            t.push(toks![
                "func (this *",
                name.clone(),
                ") UnmarshalJSON(b []byte) error {"
            ]);

            t.nested({
                let mut t = Tokens::new();

                t.push("var s string");

                t.push_into(|t| {
                    t.push(toks![
                        "if err := ",
                        codegen.unmarshal.clone(),
                        "(b, &s); err != nil {"
                    ]);
                    t.nested("return err");
                    t.push("}");
                });

                t.push_into(|t| {
                    t.push("switch s {");

                    for v in &body.variants {
                        t.push_into(|t| {
                            t.push(toks!["case ", v.ordinal().quoted(), ":"]);
                            t.nested(toks!["*this = ", name.clone(), "_", v.ident.as_str()]);
                        });
                    }

                    t.push_into(|t| {
                        t.push("default:");
                        t.nested(toks![
                            "return ",
                            codegen.new_error.clone(),
                            "(",
                            "bad value".quoted(),
                            ")"
                        ]);
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
            codegen: &Codegen,
            name: Go<'el>,
            body: &'el RpEnumBody,
        ) -> Tokens<'el, Go<'el>> {
            let mut t = Tokens::new();

            t.push(toks![
                "func (this ",
                name.clone(),
                ") MarshalJSON() ([]byte, error) {"
            ]);

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
                        t.push("default:");
                        t.nested(toks![
                            "return nil, ",
                            codegen.new_error.clone(),
                            "(",
                            "bad value".quoted(),
                            ")"
                        ]);
                    });

                    t.push("}");
                });

                t.push(toks!["return ", codegen.marshal.clone(), "(s)"]);

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

            t.push(toks![
                "func (this *",
                name.clone(),
                ") UnmarshalJSON(b []byte) error {"
            ]);

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

            Ok(t)
        }

        fn marshal_json<'el>(
            c: &Codegen,
            name: Go<'el>,
            body: &'el RpTupleBody,
        ) -> Result<Tokens<'el, Go<'el>>> {
            let mut t = Tokens::new();

            t.push(toks![
                "func (this ",
                name.clone(),
                ") MarshalJSON() ([]byte, error) {"
            ]);

            t.nested({
                let mut t = Tokens::new();

                t.push(toks!["var array []", c.raw_message.clone()]);

                t.push({
                    let mut t = Tokens::new();

                    for f in &body.fields {
                        let var = f.safe_ident();

                        t.push({
                            let mut t = Tokens::new();

                            t.push(toks![
                                var.clone(),
                                ", err := ",
                                c.marshal.clone(),
                                "(this.",
                                f.safe_ident(),
                                ")"
                            ]);

                            t.push_into(|t| {
                                t.push("if err != nil {");
                                t.nested("return nil, err");
                                t.push("}");
                            });

                            t.push(toks!["array = append(array, ", var.clone(), ")"]);

                            t.join_line_spacing()
                        });
                    }

                    t.join_line_spacing()
                });

                t.push(toks!["return ", c.marshal.clone(), "(array)"]);

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

            t.push(toks![
                "func (this ",
                name.clone(),
                ") UnmarshalJSON(b []byte) error {"
            ]);

            t.push("}");

            Ok(t)
        }

        fn marshal_json<'el>(
            c: &Codegen,
            name: Go<'el>,
            body: &'el RpInterfaceBody,
        ) -> Result<Tokens<'el, Go<'el>>> {
            let mut t = Tokens::new();

            t.push(toks![
                "func (this ",
                name.clone(),
                ") MarshalJSON() ([]byte, error) {"
            ]);

            t.push("}");

            Ok(t)
        }
    }
}
