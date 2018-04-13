//! Backend for Swift

use backend::PackageProcessor;
use core::errors::*;
use core::{Handle, Loc};
use flavored::{RpEnumBody, RpField, RpInterfaceBody, RpTupleBody, RpTypeBody, SwiftFlavor,
               SwiftName};
use genco::swift::Swift;
use genco::{IntoTokens, Tokens};
use trans::{self, Packages, Translated};
use {EnumAdded, FileSpec, InterfaceAdded, InterfaceModelAdded, Options, PackageAdded,
     StructModelAdded, TupleAdded, TypeAdded, EXT};

/// Documentation comments.
pub struct Comments<'el, S: 'el>(pub &'el [S]);

impl<'el, S: 'el + AsRef<str>> IntoTokens<'el, Swift<'el>> for Comments<'el, S> {
    fn into_tokens(self) -> Tokens<'el, Swift<'el>> {
        let mut t = Tokens::new();

        for c in self.0.iter() {
            t.push(toks!["// ", c.as_ref()]);
        }

        t
    }
}

pub struct Compiler<'el> {
    pub env: &'el Translated<SwiftFlavor>,
    options: Options,
    handle: &'el Handle,
}

impl<'el> Compiler<'el> {
    pub fn new(
        env: &'el Translated<SwiftFlavor>,
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

    pub fn into_field<'a>(&self, field: &'a RpField) -> Result<Tokens<'a, Swift<'a>>> {
        if field.is_optional() {
            return Ok(toks![field.ty().ty(), "?"]);
        }

        Ok(toks![field.ty().ty()])
    }

    /// Set up a model structure for the given fields.
    fn model_struct<'a, F>(
        &self,
        name: &SwiftName,
        comment: &'a [String],
        fields: F,
        extends: bool,
    ) -> Result<Tokens<'a, Swift<'a>>>
    where
        F: IntoIterator<Item = &'a RpField>,
    {
        let fields = fields.into_iter().collect::<Vec<_>>();

        let mut t = Tokens::new();

        t.push_unless_empty(Comments(comment));

        if self.options.struct_model_extends.is_empty() || !extends {
            t.push(toks!["public struct ", name.name.clone(), " {"]);
        } else {
            let extends = self.options.struct_model_extends.clone().join(", ");
            t.push(toks![
                "public struct ",
                name.name.clone(),
                ": ",
                extends,
                " {"
            ]);
        }

        // fields
        t.nested({
            let mut t = Tokens::new();

            t.push({
                let mut t = Tokens::new();

                for field in fields.iter() {
                    t.push_unless_empty(Comments(&field.comment));
                    let ty = self.into_field(field)?;
                    t.push(toks!["let ", field.safe_ident(), ": ", ty]);
                }

                t
            });

            for g in &self.options.struct_model_gens {
                g.generate(StructModelAdded {
                    container: &mut t,
                    fields: &fields,
                })?;
            }

            t.join_line_spacing()
        });

        t.push("}");
        Ok(t)
    }

    /// Build a model struct for the given set of fields.
    fn model_type<'a, F>(
        &self,
        name: &'a SwiftName,
        comment: &'a [String],
        fields: F,
    ) -> Result<Tokens<'a, Swift<'a>>>
    where
        F: IntoIterator<Item = &'a RpField>,
    {
        let fields = fields.into_iter().collect::<Vec<_>>();

        let mut tokens = Tokens::new();

        tokens.push(self.model_struct(name, comment, fields.iter().cloned(), true)?);

        for g in &self.options.type_gens {
            g.generate(TypeAdded {
                container: &mut tokens,
                compiler: self,
                name: name,
                fields: &fields,
            })?;
        }

        return Ok(tokens);
    }

    pub fn compile(&self, packages: &Packages) -> Result<()> {
        let mut files = self.populate_files()?;

        for g in &self.options.package_gens {
            let mut f = Vec::new();
            g.generate(PackageAdded { files: &mut f })?;

            for (package, out) in f {
                files.insert(packages.package(package)?, out);
            }
        }

        self.write_files(files)
    }
}

impl<'el> PackageProcessor<'el, SwiftFlavor, SwiftName> for Compiler<'el> {
    type Out = FileSpec<'el>;
    type DeclIter = trans::translated::DeclIter<'el, SwiftFlavor>;

    fn ext(&self) -> &str {
        EXT
    }

    fn decl_iter(&self) -> Self::DeclIter {
        self.env.decl_iter()
    }

    fn handle(&self) -> &'el Handle {
        self.handle
    }

    fn default_process(&self, _out: &mut Self::Out, _: &SwiftName) -> Result<()> {
        Ok(())
    }

    fn process_type(&self, out: &mut Self::Out, body: &'el RpTypeBody) -> Result<()> {
        out.0.extend(self.model_type(
            &body.name,
            &body.comment,
            body.fields.iter().map(Loc::borrow),
        )?);

        Ok(())
    }

    fn process_tuple(&self, out: &mut Self::Out, body: &'el RpTupleBody) -> Result<()> {
        out.0.push({
            let fields = body.fields.iter().map(Loc::borrow).collect::<Vec<_>>();

            let mut tokens = Tokens::new();

            tokens.push(self.model_struct(
                &body.name,
                &body.comment,
                fields.iter().cloned(),
                false,
            )?);

            for g in &self.options.tuple_gens {
                g.generate(TupleAdded {
                    container: &mut tokens,
                    compiler: self,
                    name: &body.name,
                    fields: &fields,
                })?;
            }

            tokens
        });

        Ok(())
    }

    fn process_enum(&self, out: &mut Self::Out, body: &'el RpEnumBody) -> Result<()> {
        out.0.push({
            let mut t = Tokens::new();

            t.push(toks!["public enum ", body.name.name.clone(), " {"]);

            for v in &body.variants {
                nested!(t, "case ", v.ident());
            }

            t.push("}");

            t
        });

        for g in &self.options.enum_gens {
            g.generate(EnumAdded {
                container: &mut out.0,
                name: &body.name,
                body: body,
            })?;
        }

        return Ok(());
    }

    fn process_interface(&self, out: &mut Self::Out, body: &'el RpInterfaceBody) -> Result<()> {
        out.0.push({
            let mut t = Tokens::new();

            t.push_unless_empty(Comments(&body.comment));
            t.push(toks!["public enum ", body.name.name.clone(), " {"]);

            t.push({
                let mut t = Tokens::new();

                t.push_into(|t| {
                    for sub_type in body.sub_types.iter() {
                        let ident = sub_type.ident.as_str();
                        t.nested(toks!["case ", ident, "(", sub_type.name.name.clone(), ")"]);
                    }
                });

                t.push_unless_empty({
                    let mut t = Tokens::new();

                    for g in &self.options.interface_model_gens {
                        g.generate(InterfaceModelAdded {
                            container: &mut t,
                            body: body,
                        })?;
                    }

                    t.join_line_spacing()
                });

                t.join_line_spacing()
            });

            t.push("}");
            t
        });

        for g in &self.options.interface_gens {
            g.generate(InterfaceAdded {
                container: &mut out.0,
                compiler: self,
                name: &body.name,
                body: body,
            })?;
        }

        for sub_type in body.sub_types.iter() {
            let fields = body.fields
                .iter()
                .chain(sub_type.fields.iter())
                .map(Loc::borrow);

            out.0
                .push(self.model_type(&sub_type.name, &sub_type.comment, fields)?);
        }

        return Ok(());
    }
}
