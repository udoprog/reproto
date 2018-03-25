//! Backend for Go

use backend::{PackageProcessor, PackageUtils};
use core::errors::*;
use core::flavored::{RpEnumBody, RpField, RpInterfaceBody, RpName, RpPackage, RpTupleBody, RpType,
                     RpTypeBody, RpVersionedPackage};
use core::{CoreFlavor, Handle, Loc, RelativePathBuf, Version};
use genco::go::{imported, local, Go};
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

const TYPE_SEP: &'static str = "_";

pub struct Compiler<'el> {
    pub env: &'el Translated<CoreFlavor>,
    options: Options,
    handle: &'el Handle,
}

impl<'el> Compiler<'el> {
    pub fn new(
        env: &'el Translated<CoreFlavor>,
        options: Options,
        handle: &'el Handle,
    ) -> Result<Compiler<'el>> {
        let c = Compiler {
            env: env,
            options: options,
            handle: handle,
        };

        Ok(c)
    }

    /// Convert the type name
    ///
    /// Optionally also emit the necessary attributes to suppress warnings for bad naming
    /// conventions.
    pub fn convert_name<'a>(&self, name: &'a RpName) -> Result<Go<'a>> {
        let registered = self.env.lookup(name)?;
        let ident = registered.ident(&name, |p| p.join(TYPE_SEP), |c| c.join(TYPE_SEP));

        // imported
        if let Some(_) = name.prefix {
            let module = self.package(&name.package).parts.join("_");
            let module = format!("../{}", module);
            return Ok(imported(module, ident));
        }

        // same package
        return Ok(local(ident));
    }

    /// Convert the given type to a Go type suitable for adding as a field to a struct.
    pub fn field_type(&self, ty: &'el RpType) -> Result<Tokens<'el, Go<'el>>> {
        use core::RpType::*;

        let ty = match *ty {
            String => toks!["string"],
            DateTime => toks!["string"],
            Bytes => toks!["string"],
            Signed { size: 32 } => toks!["int32"],
            Signed { size: 64 } => toks!["int64"],
            Unsigned { size: 32 } => toks!["uint32"],
            Unsigned { size: 64 } => toks!["uint64"],
            Float => toks!["float32"],
            Double => toks!["float64"],
            Boolean => toks!["bool"],
            Array { ref inner } => {
                let argument = self.field_type(inner)?;
                toks!["[]", argument]
            }
            Name { ref name } => toks![self.convert_name(name)?],
            Map { ref key, ref value } => {
                let key = self.field_type(key)?;
                let value = self.field_type(value)?;
                toks!["map[", key, "]", value]
            }
            Any => toks!["interface{}"],
            _ => return Err(format!("unsupported type: {}", ty).into()),
        };

        Ok(ty)
    }

    fn process_struct<'a, I>(
        &self,
        name: Go<'el>,
        comment: &'el [String],
        fields: I,
    ) -> Result<Tokens<'el, Go<'el>>>
    where
        I: IntoIterator<Item = &'el RpField>,
    {
        let mut t = Tokens::new();

        t.push(Comments(comment));
        t.push(toks!["type ", name.clone(), " struct {"]);

        t.nested({
            let mut t = Tokens::new();

            for f in fields.into_iter() {
                let ty = self.field_type(&f.ty)?;

                let ty = if f.is_optional() { toks!["*", ty] } else { ty };

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

impl<'el> PackageUtils for Compiler<'el> {
    fn version_package(input: &Version) -> String {
        input.to_string().replace(Self::package_version_unsafe, "_")
    }
}

impl<'el> PackageProcessor<'el, CoreFlavor> for Compiler<'el> {
    type Out = FileSpec<'el>;
    type DeclIter = trans::translated::DeclIter<'el, CoreFlavor>;

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

    fn default_process(&self, _out: &mut Self::Out, _: &RpName) -> Result<()> {
        Ok(())
    }

    fn resolve_full_path(&self, package: &RpPackage) -> Result<RelativePathBuf> {
        let mut full_path = RelativePathBuf::from(
            package
                .parts
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join("_"),
        ).join("lib");

        full_path.set_extension(self.ext());
        Ok(full_path)
    }

    fn process_type(&self, out: &mut Self::Out, body: &'el RpTypeBody) -> Result<()> {
        let name = self.convert_name(&body.name)?;

        out.0
            .push(self.process_struct(name, &body.comment, body.fields.iter().map(Loc::value))?);

        Ok(())
    }

    fn process_tuple(&self, out: &mut Self::Out, body: &'el RpTupleBody) -> Result<()> {
        let name = self.convert_name(&body.name)?;

        out.0.try_push_into::<Error, _>(|t| {
            t.push(Comments(&body.comment));
            t.push(toks!["type ", name.clone(), " struct {"]);

            t.nested({
                let mut t = Tokens::new();

                for f in &body.fields {
                    let ty = self.field_type(&f.ty)?;

                    let ty = if f.is_optional() { toks!["*", ty] } else { ty };

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
                name: name.clone(),
                body: body,
                compiler: self,
            })?;
        }

        Ok(())
    }

    fn process_enum(&self, out: &mut Self::Out, body: &'el RpEnumBody) -> Result<()> {
        let name = self.convert_name(&body.name)?;

        out.0.push({
            let mut t = Tokens::new();

            t.push_into(|t| {
                t.push(Comments(&body.comment));
                t.push(toks!["type ", name.clone(), " int"])
            });

            t.push_into(|t| {
                t.push("const (");
                t.nested_into(|t| {
                    let mut it = body.variants.iter();

                    if let Some(v) = it.next() {
                        t.push(toks![
                            name.clone(),
                            "_",
                            v.ident.as_str(),
                            " ",
                            name.clone(),
                            " = iota",
                        ]);
                    }

                    while let Some(v) = it.next() {
                        t.push(toks![name.clone(), "_", v.ident.as_str(),]);
                    }
                });
                t.push(")");
            });

            t.join_line_spacing()
        });

        for g in &self.options.enum_gens {
            g.generate(EnumAdded {
                container: &mut out.0,
                name: name.clone(),
                body: body,
            })?;
        }

        Ok(())
    }

    fn process_interface(&self, out: &mut Self::Out, body: &'el RpInterfaceBody) -> Result<()> {
        let name = self.convert_name(&body.name)?;

        out.0.push({
            let mut t = Tokens::new();

            t.try_push_into::<Error, _>(|t| {
                t.push(Comments(&body.comment));
                t.push(toks!["type ", name.clone(), " struct {"]);

                for sub_type in &body.sub_types {
                    let sub_name = self.convert_name(&sub_type.name)?;
                    nested!(t, sub_type.ident, " *", sub_name);
                }

                t.push("}");
                Ok(())
            })?;

            t.push({
                let mut t = Tokens::new();

                for sub_type in &body.sub_types {
                    let sub_name = self.convert_name(&sub_type.name)?;

                    t.push(self.process_struct(
                        sub_name.clone(),
                        &sub_type.comment,
                        body.fields
                            .iter()
                            .chain(sub_type.fields.iter())
                            .map(Loc::value),
                    )?);
                }

                t.join_line_spacing()
            });

            t.join_line_spacing()
        });

        for g in &self.options.interface_gens {
            g.generate(InterfaceAdded {
                container: &mut out.0,
                name: name.clone(),
                body: body,
                compiler: self,
            })?;
        }

        Ok(())
    }
}
