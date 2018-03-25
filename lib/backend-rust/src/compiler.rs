//! Backend for Rust

use backend::{PackageProcessor, PackageUtils};
use core::errors::*;
use core::flavored::{RpEnumBody, RpField, RpInterfaceBody, RpName, RpPackage, RpServiceBody,
                     RpTupleBody, RpType, RpTypeBody, RpVersionedPackage};
use core::{self, CoreFlavor, ForEachLoc, Handle, Loc, RelativePath, RelativePathBuf};
use genco::rust::{imported, imported_alias};
use genco::{Element, IntoTokens, Quoted, Rust, Tokens};
use rust_file_spec::RustFileSpec;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use trans::{self, Translated};
use {Options, EXT, MOD};

/// #[allow(non_camel_case_types)] attribute.
pub struct AllowNonCamelCaseTypes;

impl<'a> IntoTokens<'a, Rust<'a>> for AllowNonCamelCaseTypes {
    fn into_tokens(self) -> Tokens<'a, Rust<'a>> {
        "#[allow(non_camel_case_types)]".into()
    }
}

/// Serializer derives.
pub struct Derives;

impl<'a> IntoTokens<'a, Rust<'a>> for Derives {
    fn into_tokens(self) -> Tokens<'a, Rust<'a>> {
        "#[derive(Serialize, Deserialize, Debug)]".into()
    }
}

/// A serde rename annotation.
pub struct Rename<'a>(&'a str);

impl<'a> IntoTokens<'a, Rust<'a>> for Rename<'a> {
    fn into_tokens(self) -> Tokens<'a, Rust<'a>> {
        toks!["#[serde(rename = ", self.0.quoted(), ")]"]
    }
}

/// Tag attribute.
pub struct Tag<'a>(&'a str);

impl<'a> IntoTokens<'a, Rust<'a>> for Tag<'a> {
    fn into_tokens(self) -> Tokens<'a, Rust<'a>> {
        toks!["#[serde(tag = ", self.0.quoted(), ")]"]
    }
}

/// Documentation comments.
pub struct Comments<'el, S: 'el>(&'el [S]);

impl<'el, S: 'el + AsRef<str>> IntoTokens<'el, Rust<'el>> for Comments<'el, S> {
    fn into_tokens(self) -> Tokens<'el, Rust<'el>> {
        let mut t = Tokens::new();

        for c in self.0.iter() {
            t.push(toks!["/// ", c.as_ref()]);
        }

        t
    }
}

const TYPE_SEP: &'static str = "_";
const SCOPE_SEP: &'static str = "::";

pub struct Compiler<'el> {
    pub env: &'el Translated<CoreFlavor>,
    handle: &'el Handle,
    hash_map: Rust<'static>,
    json_value: Rust<'static>,
    datetime: Option<Tokens<'static, Rust<'static>>>,
}

impl<'el> Compiler<'el> {
    pub fn new(
        env: &'el Translated<CoreFlavor>,
        options: Options,
        handle: &'el Handle,
    ) -> Compiler<'el> {
        Compiler {
            env: env,
            handle: handle,
            hash_map: imported("std::collections", "HashMap"),
            json_value: imported_alias("serde_json", "Value", "json"),
            datetime: options.datetime.clone(),
        }
    }

    /// Build an implementation of the given name and body.
    fn build_impl(&self, name: Rc<String>, body: Tokens<'el, Rust<'el>>) -> Tokens<'el, Rust<'el>> {
        let mut out_impl = Tokens::new();

        out_impl.push(toks!["impl ", name.clone(), " {"]);
        out_impl.nested(body);
        out_impl.push("}");

        out_impl
    }

    /// Convert the type name
    ///
    /// Optionally also emit the necessary attributes to suppress warnings for bad naming
    /// conventions.
    fn convert_type_name(&self, name: &RpName) -> (Rc<String>, Tokens<'static, Rust<'static>>) {
        let attributes = if name.parts.len() > 1 {
            AllowNonCamelCaseTypes.into_tokens()
        } else {
            Tokens::new()
        };

        (Rc::new(name.join(TYPE_SEP)), attributes)
    }

    fn convert_type_id<'a>(&self, name: &'a RpName) -> Result<Element<'a, Rust<'a>>> {
        let registered = self.env.lookup(name)?;

        let ident = registered.ident(&name, |p| p.join(TYPE_SEP), |c| c.join(SCOPE_SEP));

        if let Some(ref prefix) = name.prefix {
            let package_name = self.package(&name.package).parts.join("::");
            return Ok(imported_alias(package_name, ident, prefix.as_str()).into());
        }

        Ok(ident.into())
    }

    fn into_type<'a>(&self, field: &'a RpField) -> Result<Tokens<'a, Rust<'a>>> {
        let stmt = self.into_rust_type(&field.ty)?;

        if field.is_optional() {
            return Ok(toks!["Option<", stmt, ">"]);
        }

        Ok(stmt)
    }

    fn enum_value_fn<'a>(
        &self,
        name: Rc<String>,
        match_body: Tokens<'a, Rust<'a>>,
    ) -> Tokens<'a, Rust<'a>> {
        let mut value_fn = Tokens::new();
        let mut match_decl = Tokens::new();

        match_decl.push("match *self {");
        match_decl.nested(match_body);
        match_decl.push("}");

        value_fn.push("pub fn value(&self) -> &'static str {");
        value_fn.nested(toks!["use self::", name, "::*;"]);
        value_fn.nested(match_decl);
        value_fn.push("}");

        value_fn
    }

    fn datetime<'a>(&self, _ty: &RpType) -> Result<Tokens<'a, Rust<'a>>> {
        if let Some(ref datetime) = self.datetime {
            return Ok(datetime.clone().into());
        }

        Err("Missing implementation for `datetime`, try: -m chrono".into())
    }

    pub fn into_rust_type<'a>(&self, ty: &'a RpType) -> Result<Tokens<'a, Rust<'a>>> {
        use core::RpType::*;

        let ty = match *ty {
            String => toks!["String"],
            DateTime => self.datetime(ty)?,
            Bytes => toks!["String"],
            Signed { size: 32 } => toks!["i32"],
            Signed { size: 64 } => toks!["i64"],
            Unsigned { size: 32 } => toks!["u32"],
            Unsigned { size: 64 } => toks!["u64"],
            Float => toks!["f32"],
            Double => toks!["f64"],
            Boolean => toks!["bool"],
            Array { ref inner } => {
                let argument = self.into_rust_type(inner)?;
                toks!["Vec<", argument, ">"]
            }
            Name { ref name } => toks![self.convert_type_id(name)?],
            Map { ref key, ref value } => {
                let key = self.into_rust_type(key)?;
                let value = self.into_rust_type(value)?;
                toks![self.hash_map.clone(), "<", key, ", ", value, ">"]
            }
            Any => toks![self.json_value.clone()],
            _ => return Err(format!("unsupported type: {}", ty).into()),
        };

        Ok(ty)
    }

    // Build the corresponding element out of a field declaration.
    fn field_element<'a>(&self, field: &'a RpField) -> Result<Tokens<'a, Rust<'a>>> {
        let mut t = Tokens::new();

        let ident = field.safe_ident();
        let type_spec = self.into_type(field)?;

        if field.is_optional() {
            t.push(toks!["#[serde(skip_serializing_if=\"Option::is_none\")]"]);
        }

        if field.name() != ident {
            t.push(Rename(field.name()));
        }

        t.push(toks![ident, ": ", type_spec, ","]);

        Ok(t.into())
    }

    pub fn compile(&self) -> Result<()> {
        let files = self.populate_files()?;
        self.write_mod_files(&files)?;
        self.write_files(files)
    }

    fn write_mod_files(&self, files: &BTreeMap<RpVersionedPackage, RustFileSpec>) -> Result<()> {
        let mut packages: BTreeMap<RelativePathBuf, BTreeSet<String>> = BTreeMap::new();
        let mut root_names = BTreeSet::new();

        for (key, _) in files {
            let mut current = RelativePathBuf::new();

            let mut it = self.package(key).parts.into_iter().peekable();

            if let Some(root) = it.peek() {
                root_names.insert(root.to_owned());
            }

            while let Some(part) = it.next() {
                current = current.join(part);

                if let Some(next) = it.peek() {
                    let mut full_path = current.join(MOD);
                    full_path.set_extension(self.ext());

                    packages
                        .entry(full_path)
                        .or_insert_with(BTreeSet::new)
                        .insert(next.clone());
                }
            }
        }

        let mut root_mod = RelativePathBuf::new().join(MOD);
        root_mod.set_extension(self.ext());
        packages.insert(root_mod, root_names);

        let handle = self.handle();

        for (full_path, children) in packages {
            let parent = full_path.parent().unwrap_or(RelativePath::new("."));

            if !self.handle.is_dir(&parent) {
                debug!("+dir: {}", parent.display());
                handle.create_dir_all(&parent)?;
            }

            if !handle.is_file(&full_path) {
                debug!("+mod: {}", full_path.display());
                let mut f = handle.create(&full_path)?;

                for child in children {
                    writeln!(f, "pub mod {};", child)?;
                }
            }
        }

        Ok(())
    }
}

impl<'el> PackageUtils for Compiler<'el> {}

impl<'el> PackageProcessor<'el, CoreFlavor> for Compiler<'el> {
    type Out = RustFileSpec<'el>;
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

    fn process_tuple(&self, out: &mut Self::Out, body: &'el RpTupleBody) -> Result<()> {
        let mut fields = Tokens::new();

        for field in &body.fields {
            fields.push(self.into_type(field)?);
        }

        let (name, attributes) = self.convert_type_name(&body.name);

        let mut t = Tokens::new();

        t.push_unless_empty(Comments(&body.comment));
        t.push_unless_empty(attributes);
        t.push(Derives);
        t.push(toks!["struct ", name, "(", fields.join(", "), ");",]);

        out.0.push(t);
        Ok(())
    }

    fn process_enum(&self, out: &mut Self::Out, body: &'el RpEnumBody) -> Result<()> {
        let (name, attributes) = self.convert_type_name(&body.name);

        // variant declarations
        let mut variants = Tokens::new();
        // body of value function
        let mut match_body = Tokens::new();

        body.variants.iter().for_each_loc(|variant| {
            let value = if let core::RpEnumOrdinal::String(ref s) = variant.ordinal {
                if s != variant.ident() {
                    variants.push(Rename(s.as_str()));
                }

                s
            } else {
                variant.ident()
            };

            match_body.push(toks![variant.ident(), " => ", value.quoted(), ",",]);

            variants.push_unless_empty(Comments(&variant.comment));
            variants.push(toks![variant.ident(), ","]);
            Ok(()) as Result<()>
        })?;

        out.0.push({
            let mut t = Tokens::new();

            t.push_unless_empty(Comments(&body.comment));
            t.push_unless_empty(attributes);
            t.push(Derives);
            t.push(toks!["pub enum ", name.clone(), " {"]);
            t.nested(variants);
            t.push("}");

            t
        });

        out.0.push({
            let mut t = Tokens::new();

            t.push(toks!["impl ", name.clone(), " {"]);

            t.nested({
                let mut t = Tokens::new();
                t.push(self.enum_value_fn(name.clone(), match_body));
                t.push_unless_empty(code!(&body.codes, core::RpContext::Rust));
                t
            });

            t.push("}");

            t
        });

        Ok(())
    }

    fn process_type(&self, out: &mut Self::Out, body: &'el RpTypeBody) -> Result<()> {
        let (name, attributes) = self.convert_type_name(&body.name);
        let mut t = Tokens::new();

        t.push_unless_empty(Comments(&body.comment));
        t.push_unless_empty(attributes);
        t.push(Derives);
        t.push(toks!["pub struct ", name.clone(), " {"]);

        // fields
        t.nested({
            let mut t = Tokens::new();

            for field in &body.fields {
                t.push_unless_empty(Comments(&field.comment));
                t.push(self.field_element(field)?);
            }

            t
        });

        t.push("}");

        out.0.push(t);

        // if custom code is present, punt it into an impl.
        let impl_body = code!(&body.codes, core::RpContext::Rust).into_tokens();

        if !impl_body.is_empty() {
            out.0.push(self.build_impl(name.clone(), impl_body));
        }

        Ok(())
    }

    fn process_interface(&self, out: &mut Self::Out, body: &'el RpInterfaceBody) -> Result<()> {
        let (name, attributes) = self.convert_type_name(&body.name);

        let mut t = Tokens::new();

        t.push_unless_empty(Comments(&body.comment));
        t.push_unless_empty(attributes);
        t.push(Derives);

        match body.sub_type_strategy {
            core::RpSubTypeStrategy::Tagged { ref tag, .. } => {
                t.push(Tag(tag.as_str()));
            }
        }

        t.push(toks!["pub enum ", name.clone(), " {"]);

        body.sub_types.iter().for_each_loc(|s| {
            t.nested({
                let mut t = Tokens::new();

                t.push_unless_empty(Comments(&s.comment));

                // TODO: clone should not be needed
                if let Some(ref name) = s.sub_type_name {
                    if name.as_str() != s.ident.as_str() {
                        t.push(Rename(name));
                    }
                }

                t.push(toks![s.ident.as_str(), " {"]);

                for field in body.fields.iter().chain(s.fields.iter()) {
                    t.nested(self.field_element(field)?);
                }

                t.push("},");

                t
            });

            Ok(()) as Result<()>
        })?;

        t.push("}");

        out.0.push(t);

        let impl_body = code!(&body.codes, core::RpContext::Rust).into_tokens();

        if !impl_body.is_empty() {
            out.0.push(self.build_impl(name.clone(), impl_body));
        }

        Ok(())
    }

    fn process_service(&self, out: &mut Self::Out, body: &'el RpServiceBody) -> Result<()> {
        let (name, attributes) = self.convert_type_name(&body.name);
        let mut t = Tokens::new();

        t.push_unless_empty(Comments(&body.comment));
        t.push_unless_empty(attributes);
        t.push(toks!["pub trait ", name.clone(), " {"]);

        let endpoints = body.endpoints.iter().map(Loc::as_ref);

        endpoints.for_each_loc(|e| {
            t.nested({
                let mut t = Tokens::new();
                t.push_unless_empty(Comments(&e.comment));
                t.push(toks!["fn ", e.safe_ident(), "();"]);
                t
            });

            Ok(()) as Result<()>
        })?;

        t.push("}");

        out.0.push(t);

        Ok(())
    }
}
