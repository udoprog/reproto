//! Backend for Swift

use backend::{PackageProcessor, PackageUtils};
use core::errors::*;
use core::flavored::{RpEnumBody, RpField, RpInterfaceBody, RpName, RpPackage, RpTupleBody, RpType,
                     RpTypeBody, RpVersionedPackage};
use core::{CoreFlavor, Handle, Loc};
use genco::swift::{self, Swift};
use genco::{IntoTokens, Tokens};
use trans::{self, Translated};
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

const TYPE_SEP: &'static str = "_";

pub struct Compiler<'el> {
    pub env: &'el Translated<CoreFlavor>,
    options: Options,
    handle: &'el Handle,
    data: Swift<'static>,
    date: Swift<'static>,
    any: Tokens<'static, Swift<'static>>,
}

impl<'el> Compiler<'el> {
    pub fn new(
        env: &'el Translated<CoreFlavor>,
        options: Options,
        handle: &'el Handle,
    ) -> Result<Compiler<'el>> {
        let any = {
            let mut any_types = options.any_type.iter().cloned();

            if let Some((first_mod, any_type)) = any_types.next() {
                if let Some((second_mod, _)) = any_types.next() {
                    return Err(format!(
                        "Any type provided by more than one module: {}, {}",
                        first_mod, second_mod
                    ).into());
                }

                toks![any_type.clone()]
            } else {
                toks!["Any"]
            }
        };

        let c = Compiler {
            env: env,
            options: options,
            handle: handle,
            data: swift::imported("Foundation", "Data"),
            date: swift::imported("Foundation", "Date"),
            any: any,
        };

        Ok(c)
    }

    /// Convert the type name
    ///
    /// Optionally also emit the necessary attributes to suppress warnings for bad naming
    /// conventions.
    pub fn convert_name<'a>(&self, name: &'a RpName) -> Result<Swift<'a>> {
        let registered = self.env.lookup(name)?;
        let ident = registered.ident(&name, |p| p.join(TYPE_SEP), |c| c.join(TYPE_SEP));
        let package_name = self.package(&name.package).parts.join("_");
        return Ok(swift::local(format!("{}_{}", package_name, ident)));
    }

    /// Convert to the type declaration of a field.
    pub fn field_type<'a>(&self, ty: &'a RpType) -> Result<Tokens<'a, Swift<'a>>> {
        use core::RpType::*;

        let ty = match *ty {
            String => toks!["String"],
            DateTime => toks![self.date.clone()],
            Bytes => toks![self.data.clone()],
            Signed { size: 32 } => toks!["Int32"],
            Signed { size: 64 } => toks!["Int64"],
            Unsigned { size: 32 } => toks!["UInt32"],
            Unsigned { size: 64 } => toks!["UInt64"],
            Float => toks!["Float"],
            Double => toks!["Double"],
            Boolean => toks!["Bool"],
            Array { ref inner } => {
                let argument = self.field_type(inner)?;
                toks!["[", argument, "]"]
            }
            Name { ref name } => toks![self.convert_name(name)?],
            Map { ref key, ref value } => {
                let key = self.field_type(key)?;
                let value = self.field_type(value)?;
                toks!["[", key, ": ", value, "]"]
            }
            Any => toks![self.any.clone()],
            _ => return Err(format!("unsupported type: {}", ty).into()),
        };

        Ok(ty)
    }

    pub fn into_field<'a>(&self, field: &'a RpField) -> Result<Tokens<'a, Swift<'a>>> {
        let stmt = self.field_type(&field.ty)?;

        if field.is_optional() {
            return Ok(toks![stmt, "?"]);
        }

        Ok(stmt)
    }

    /// Set up a model structure for the given fields.
    fn model_struct<'a, F>(
        &self,
        name: Swift<'a>,
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
            t.push(toks!["public struct ", name.clone(), " {"]);
        } else {
            let extends = self.options.struct_model_extends.clone().join(", ");
            t.push(toks!["public struct ", name.clone(), ": ", extends, " {"]);
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
        name: Swift<'a>,
        comment: &'a [String],
        fields: F,
    ) -> Result<Tokens<'a, Swift<'a>>>
    where
        F: IntoIterator<Item = &'a RpField>,
    {
        let fields = fields.into_iter().collect::<Vec<_>>();

        let mut tokens = Tokens::new();

        tokens.push(self.model_struct(name.clone(), comment, fields.iter().cloned(), true)?);

        for g in &self.options.type_gens {
            g.generate(TypeAdded {
                container: &mut tokens,
                compiler: self,
                name: &name,
                fields: &fields,
            })?;
        }

        return Ok(tokens);
    }

    pub fn compile(&self) -> Result<()> {
        let mut files = self.populate_files()?;

        for g in &self.options.package_gens {
            g.generate(PackageAdded { files: &mut files })?;
        }

        self.write_files(files)
    }
}

impl<'el> PackageUtils for Compiler<'el> {}

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

    fn process_type(&self, out: &mut Self::Out, body: &'el RpTypeBody) -> Result<()> {
        let name = self.convert_name(&body.name)?;

        out.0
            .extend(self.model_type(name, &body.comment, body.fields.iter().map(Loc::value))?);

        Ok(())
    }

    fn process_tuple(&self, out: &mut Self::Out, body: &'el RpTupleBody) -> Result<()> {
        let name = self.convert_name(&body.name)?;

        out.0.push({
            let fields = body.fields.iter().map(Loc::value).collect::<Vec<_>>();

            let mut tokens = Tokens::new();

            tokens.push(self.model_struct(
                name.clone(),
                &body.comment,
                fields.iter().cloned(),
                false,
            )?);

            for g in &self.options.tuple_gens {
                g.generate(TupleAdded {
                    container: &mut tokens,
                    compiler: self,
                    name: &name,
                    fields: &fields,
                })?;
            }

            tokens
        });

        Ok(())
    }

    fn process_enum(&self, out: &mut Self::Out, body: &'el RpEnumBody) -> Result<()> {
        let name = self.convert_name(&body.name)?;

        out.0.push({
            let mut t = Tokens::new();

            t.push(toks!["public enum ", name.clone(), " {"]);

            for variant in &body.variants {
                t.nested(toks!["case ", variant.ident()]);
            }

            t.push("}");

            t
        });

        for g in &self.options.enum_gens {
            g.generate(EnumAdded {
                container: &mut out.0,
                name: &name,
                body: body,
            })?;
        }

        return Ok(());
    }

    fn process_interface(&self, out: &mut Self::Out, body: &'el RpInterfaceBody) -> Result<()> {
        let name = self.convert_name(&body.name)?;

        out.0.push({
            let mut t = Tokens::new();

            t.push_unless_empty(Comments(&body.comment));
            t.push(toks!["public enum ", name.clone(), " {"]);

            for sub_type in body.sub_types.iter() {
                let name = self.convert_name(&sub_type.name)?;
                let ident = sub_type.ident.as_str();
                t.nested(toks!["case ", ident, "(", name.clone(), ")"]);
            }

            for g in &self.options.interface_model_gens {
                g.generate(InterfaceModelAdded {
                    container: &mut t,
                    body: body,
                })?;
            }

            t.push("}");
            t
        });

        for g in &self.options.interface_gens {
            g.generate(InterfaceAdded {
                container: &mut out.0,
                compiler: self,
                name: &name,
                body: body,
            })?;
        }

        for sub_type in body.sub_types.iter() {
            let sub_type_name = self.convert_name(&sub_type.name)?;

            let fields = body.fields
                .iter()
                .chain(sub_type.fields.iter())
                .map(Loc::value);

            out.0
                .push(self.model_type(sub_type_name, &sub_type.comment, fields)?);
        }

        return Ok(());
    }
}
