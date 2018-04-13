//! Backend for Go

use backend::PackageProcessor;
use core::errors::*;
use core::{Handle, Loc, RelativePathBuf};
use flavored::{GoFlavor, GoName, RpEnumBody, RpField, RpInterfaceBody, RpPackage, RpTupleBody,
               RpTypeBody};
use genco::go::Go;
use genco::{IntoTokens, Tokens};
use trans::{self, Translated};
use {EnumAdded, FieldAdded, FileSpec, InterfaceAdded, Options, Tags, TupleAdded, EXT};

/// Documentation comments.
pub struct Comments<'el, S: 'el>(pub &'el [S]);

impl<'el, S: 'el + AsRef<str>> IntoTokens<'el, Go<'el>> for Comments<'el, S> {
    fn into_tokens(self) -> Tokens<'el, Go<'el>> {
        let mut t = Tokens::new();

        for c in self.0.iter() {
            t.push(toks!["// ", c.as_ref()]);
        }

        t
    }
}

pub struct Compiler<'el> {
    pub env: &'el Translated<GoFlavor>,
    options: Options,
    handle: &'el Handle,
}

impl<'el> Compiler<'el> {
    pub fn new(
        env: &'el Translated<GoFlavor>,
        options: Options,
        handle: &'el Handle,
    ) -> Result<Compiler<'el>> {
        let c = Compiler {
            env,
            options,
            handle,
        };

        Ok(c)
    }

    fn process_struct<'a, I>(
        &self,
        name: &'el GoName,
        comment: &'el [String],
        fields: I,
    ) -> Result<Tokens<'el, Go<'el>>>
    where
        I: IntoIterator<Item = &'el RpField>,
    {
        let mut t = Tokens::new();

        t.push(Comments(comment));
        t.push(toks!["type ", name, " struct {"]);

        t.nested({
            let mut t = Tokens::new();

            for f in fields.into_iter() {
                let ty = if f.is_optional() {
                    toks!["*", f.ty.clone()]
                } else {
                    toks![f.ty.clone()]
                };

                let mut tags = Tags::new();

                for g in &self.options.field_gens {
                    g.generate(FieldAdded {
                        tags: &mut tags,
                        field: f,
                    })?;
                }

                let mut base = toks![f.safe_ident(), ty];
                base.append_unless_empty(tags);

                t.push_into(|t| {
                    t.push(Comments(&f.comment));
                    t.push(base.join_spacing());
                });
            }

            t.join_line_spacing()
        });

        t.push("}");
        Ok(t)
    }

    pub fn compile(&self) -> Result<()> {
        let files = self.populate_files()?;
        self.write_files(files)
    }
}

impl<'el> PackageProcessor<'el, GoFlavor, GoName> for Compiler<'el> {
    type Out = FileSpec<'el>;
    type DeclIter = trans::translated::DeclIter<'el, GoFlavor>;

    fn ext(&self) -> &str {
        EXT
    }

    fn decl_iter(&self) -> Self::DeclIter {
        self.env.decl_iter()
    }

    fn handle(&self) -> &'el Handle {
        self.handle
    }

    fn default_process(&self, _out: &mut Self::Out, _: &GoName) -> Result<()> {
        Ok(())
    }

    fn resolve_full_path(&self, package: &RpPackage) -> Result<RelativePathBuf> {
        let mut full_path = RelativePathBuf::from(package.join("_")).join("lib");
        full_path.set_extension(self.ext());
        Ok(full_path)
    }

    fn process_type(&self, out: &mut Self::Out, body: &'el RpTypeBody) -> Result<()> {
        out.0.push(self.process_struct(
            &body.name,
            &body.comment,
            body.fields.iter().map(Loc::borrow),
        )?);

        Ok(())
    }

    fn process_tuple(&self, out: &mut Self::Out, body: &'el RpTupleBody) -> Result<()> {
        out.0.try_push_into::<Error, _>(|t| {
            t.push(Comments(&body.comment));
            t.push(toks!["type ", &body.name, " struct {"]);

            t.nested({
                let mut t = Tokens::new();

                for f in &body.fields {
                    let ty = if f.is_optional() {
                        toks!["*", f.ty.clone()]
                    } else {
                        toks![f.ty.clone()]
                    };

                    let mut tags = Tags::new();

                    let mut base = toks![f.safe_ident(), ty];
                    base.append_unless_empty(tags);

                    t.push_into(|t| {
                        t.push(Comments(&f.comment));
                        t.push(base.join_spacing());
                    });
                }

                t.join_line_spacing()
            });

            t.push("}");
            Ok(())
        })?;

        for g in &self.options.tuple_gens {
            g.generate(TupleAdded {
                container: &mut out.0,
                name: &body.name,
                body: body,
            })?;
        }

        Ok(())
    }

    fn process_enum(&self, out: &mut Self::Out, body: &'el RpEnumBody) -> Result<()> {
        out.0.push({
            let mut t = Tokens::new();

            t.push_into(|t| {
                t.push(Comments(&body.comment));
                t.push(toks!["type ", &body.name, " int"])
            });

            t.push_into(|t| {
                t.push("const (");
                t.nested_into(|t| {
                    let mut it = body.variants.iter();

                    if let Some(v) = it.next() {
                        t.push(toks![
                            &body.name,
                            "_",
                            v.ident.as_str(),
                            " ",
                            &body.name,
                            " = iota",
                        ]);
                    }

                    while let Some(v) = it.next() {
                        t.push(toks![&body.name, "_", v.ident.as_str(),]);
                    }
                });
                t.push(")");
            });

            t.join_line_spacing()
        });

        for g in &self.options.enum_gens {
            g.generate(EnumAdded {
                container: &mut out.0,
                name: &body.name,
                body: body,
            })?;
        }

        Ok(())
    }

    fn process_interface(&self, out: &mut Self::Out, body: &'el RpInterfaceBody) -> Result<()> {
        out.0.push({
            let mut t = Tokens::new();

            t.try_push_into::<Error, _>(|t| {
                t.push_unless_empty(Comments(&body.comment));
                push!(t, "type ", &body.name, " struct {");

                t.nested_into(|t| {
                    push!(t, "Value interface {");
                    nested!(t, "Is", &body.name, "()");
                    push!(t, "}");
                });

                push!(t, "}");
                Ok(())
            })?;

            t.push({
                let mut t = Tokens::new();

                for sub_type in &body.sub_types {
                    t.push(self.process_struct(
                        &sub_type.name,
                        &sub_type.comment,
                        body.fields
                            .iter()
                            .chain(sub_type.fields.iter())
                            .map(Loc::borrow),
                    )?);

                    t.push_into(|t| {
                        push!(t, "func (this ", &sub_type.name, ") Is", &body.name, "() {");
                        push!(t, "}");
                    });
                }

                t.join_line_spacing()
            });

            t.join_line_spacing()
        });

        for g in &self.options.interface_gens {
            g.generate(InterfaceAdded {
                container: &mut out.0,
                name: &body.name,
                body: body,
            })?;
        }

        Ok(())
    }
}
