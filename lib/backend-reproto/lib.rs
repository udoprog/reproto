#[macro_use]
extern crate genco;
#[macro_use]
extern crate log;
extern crate reproto_backend as backend;
extern crate reproto_core as core;
extern crate reproto_lexer as lexer;
#[macro_use]
extern crate reproto_manifest as manifest;
extern crate reproto_trans as trans;
extern crate toml;

use core::errors::Result;
use core::flavored::{RpDecl, RpEndpoint, RpEnumBody, RpField, RpInterfaceBody, RpServiceBody,
                     RpTupleBody, RpTypeBody, RpVariant};
use core::{Context, CoreFlavor, RelativePathBuf, DEFAULT_TAG};
use genco::{Custom, Formatter, IntoTokens, IoFmt, Quoted, Tokens, WriteTokens};
use manifest::{Lang, Manifest, NoModule, TryFromToml};
use std::any::Any;
use std::fmt::{self, Write};
use std::path::Path;
use std::rc::Rc;
use trans::Environment;

pub struct Comments<'el, S: 'el>(&'el [S]);

impl<'el, S> IntoTokens<'el, Reproto> for Comments<'el, S>
where
    S: AsRef<str>,
{
    fn into_tokens(self) -> Tokens<'el, Reproto> {
        let mut t = Tokens::new();

        for line in self.0 {
            let line = line.as_ref();

            if line.is_empty() {
                t.push("///");
            } else {
                t.push(toks!["/// ", line]);
            }
        }

        t
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct ReprotoLang;

impl Lang for ReprotoLang {
    lang_base!(ReprotoModule, compile);

    fn comment(&self, input: &str) -> Option<String> {
        Some(format!("//{}", input.to_string()))
    }
}

#[derive(Debug)]
pub enum ReprotoModule {
}

impl TryFromToml for ReprotoModule {
    fn try_from_string(path: &Path, id: &str, value: String) -> Result<Self> {
        NoModule::illegal(path, id, value)
    }

    fn try_from_value(path: &Path, id: &str, value: toml::Value) -> Result<Self> {
        NoModule::illegal(path, id, value)
    }
}

#[derive(Clone)]
pub enum Reproto {
}

impl Custom for Reproto {
    type Extra = ();

    fn quote_string(out: &mut Formatter, input: &str) -> fmt::Result {
        out.write_char('"')?;

        for c in input.chars() {
            match c {
                '\t' => out.write_str("\\t")?,
                '\u{0007}' => out.write_str("\\b")?,
                '\n' => out.write_str("\\n")?,
                '\r' => out.write_str("\\r")?,
                '\u{0014}' => out.write_str("\\f")?,
                '\'' => out.write_str("\\'")?,
                '"' => out.write_str("\\\"")?,
                '\\' => out.write_str("\\\\")?,
                c => out.write_char(c)?,
            }
        }

        out.write_char('"')?;

        Ok(())
    }
}

/// Compile to a reproto manifest.
fn compile(ctx: Rc<Context>, env: Environment<CoreFlavor>, manifest: Manifest) -> Result<()> {
    let env = env.translate_default()?;
    let handle = ctx.filesystem(manifest.output.as_ref().map(AsRef::as_ref))?;

    let root = RelativePathBuf::from(".");

    for (package, file) in env.for_each_file() {
        let mut path = package
            .package
            .parts
            .iter()
            .fold(root.clone(), |path, part| path.join(part));

        let parent = path.parent()
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| root.clone());

        if !handle.is_dir(&parent) {
            debug!("+dir: {}", parent.display());
            handle.create_dir_all(&parent)?;
        }

        let path = if let Some(version) = package.version.as_ref() {
            let stem = path.file_stem()
                .ok_or_else(|| format!("Missing file stem: {}", path.display()))?;

            let file_name = format!("{}-{}.reproto", stem, version);
            path.with_file_name(file_name)
        } else {
            path.with_extension("reproto")
        };

        let mut body = Tokens::new();

        for decl in &file.decls {
            body.push(format(decl)?);
        }

        let body = body.join_line_spacing();

        debug!("+file: {}", path.display());
        IoFmt(&mut handle.create(&path)?).write_file(body, &mut ())?;
    }

    Ok(())
}

/// Format a single declaration as a reproto specification.
pub fn format<'el>(decl: &'el RpDecl) -> Result<Tokens<'el, Reproto>> {
    let result = match *decl {
        core::RpDecl::Type(ref type_) => format_type(type_),
        core::RpDecl::Interface(ref interface) => format_interface(interface),
        core::RpDecl::Tuple(ref tuple) => format_tuple(tuple),
        core::RpDecl::Enum(ref en) => format_enum(en),
        core::RpDecl::Service(ref service) => format_service(service),
    };

    return result;

    fn format_type<'el>(body: &'el RpTypeBody) -> Result<Tokens<'el, Reproto>> {
        let mut t = Tokens::new();

        t.push_unless_empty(Comments(&body.comment));
        t.push(toks!["type ", body.ident.as_str(), " {"]);

        t.nested({
            let mut t = Tokens::new();

            for f in &body.fields {
                t.push(format_field(f)?);
            }

            for d in &body.decls {
                t.push(format(d)?);
            }

            t.join_line_spacing()
        });

        t.push("}");

        Ok(t)
    }

    fn format_interface<'el>(body: &'el RpInterfaceBody) -> Result<Tokens<'el, Reproto>> {
        let mut t = Tokens::new();

        match body.sub_type_strategy {
            core::RpSubTypeStrategy::Tagged { ref tag, .. } => {
                if tag != DEFAULT_TAG {
                    t.push(toks![
                        "#[type_info(strategy = ",
                        "tagged".quoted(),
                        ", tag = ",
                        tag.as_str().quoted(),
                        ")]",
                    ]);
                }
            }
        }

        t.push_unless_empty(Comments(&body.comment));
        t.push(toks!["interface ", body.ident.as_str(), " {"]);

        t.nested({
            let mut t = Tokens::new();

            for sub_type in body.sub_types.iter() {
                t.push({
                    let mut t = Tokens::new();

                    if let Some(ref alias) = sub_type.sub_type_name {
                        t.push(toks![
                            sub_type.ident.as_str(),
                            " as ",
                            alias.as_str().quoted(),
                            " {",
                        ]);
                    } else {
                        t.push(toks![sub_type.ident.as_str(), " {"]);
                    }

                    t.nested({
                        let mut t = Tokens::new();

                        for f in &sub_type.fields {
                            t.push(format_field(f)?);
                        }

                        for d in &sub_type.decls {
                            t.push(format(d)?);
                        }

                        t.join_line_spacing()
                    });

                    t.push("}");

                    t
                });
            }

            for d in &body.decls {
                t.push(format(d)?);
            }

            t.join_line_spacing()
        });

        t.push("}");

        Ok(t)
    }

    fn format_tuple<'el>(body: &'el RpTupleBody) -> Result<Tokens<'el, Reproto>> {
        let mut t = Tokens::new();

        t.push_unless_empty(Comments(&body.comment));
        t.push(toks!["tuple ", body.ident.as_str(), " {"]);

        t.nested({
            let mut t = Tokens::new();

            for f in &body.fields {
                t.push(format_field(f)?);
            }

            for d in &body.decls {
                t.push(format(d)?);
            }

            t.join_line_spacing()
        });

        t.push("}");

        Ok(t)
    }

    fn format_enum<'el>(body: &'el RpEnumBody) -> Result<Tokens<'el, Reproto>> {
        let mut t = Tokens::new();

        t.push_unless_empty(Comments(&body.comment));

        match body.enum_type {
            core::RpEnumType::String => {
                t.push(toks!["enum ", body.ident.as_str(), " as string {"]);
            }
        }

        t.nested({
            let mut t = Tokens::new();

            for v in &body.variants {
                t.push(format_variant(v)?);
            }

            t.join_line_spacing()
        });

        t.push("}");

        Ok(t)
    }

    fn format_service<'el>(body: &'el RpServiceBody) -> Result<Tokens<'el, Reproto>> {
        let mut t = Tokens::new();
        t.push_unless_empty(Comments(&body.comment));
        t.push(toks!["service ", body.ident.as_str(), " {"]);

        t.nested({
            let mut t = Tokens::new();

            for e in &body.endpoints {
                t.push_unless_empty(Comments(&e.comment));
                t.push(format_endpoint(e)?);
            }

            t.join_line_spacing()
        });

        t.push("}");

        return Ok(t);

        fn format_endpoint<'el>(e: &'el RpEndpoint) -> Result<Tokens<'el, Reproto>> {
            let mut t = Tokens::new();

            t.append(e.ident.as_str());
            t.append("(");
            t.append({
                let mut t = Tokens::new();

                for a in &e.arguments {
                    t.append({
                        let mut t = Tokens::new();

                        t.append(a.ident.as_str());
                        t.append(": ");
                        t.append(a.channel.to_string());

                        t
                    });
                }

                t.join(", ")
            });
            t.append(")");

            Ok(t)
        }
    }

    fn format_field<'el>(field: &'el RpField) -> Result<Tokens<'el, Reproto>> {
        let mut t = Tokens::new();

        t.push_unless_empty(Comments(&field.comment));

        let field_name = field.safe_ident();

        let field_name = match lexer::match_keyword(field_name) {
            Some(token) => token
                .keyword_safe()
                .ok_or_else(|| format!("keyword does not have a safe variant: {}", field_name))?,
            None => field_name,
        };

        if field.is_optional() {
            t.push(toks![field_name, "?: ", field.ty.to_string()]);
        } else {
            t.push(toks![field_name, ": ", field.ty.to_string()]);
        }

        if let Some(ref field_as) = field.field_as {
            t.extend(toks![" as ", field_as.as_str().quoted()]);
        }

        t.append(";");

        Ok(t)
    }

    fn format_variant<'el>(variant: &'el RpVariant) -> Result<Tokens<'el, Reproto>> {
        let mut t = Tokens::new();

        t.push_unless_empty(Comments(&variant.comment));

        t.push_into(|t| {
            t.append(variant.ident());

            match variant.ordinal {
                core::RpEnumOrdinal::Generated => {}
                core::RpEnumOrdinal::String(ref string) => {
                    t.append(" as ");
                    t.append(string.as_str().quoted());
                }
            }

            t.append(";");
        });

        Ok(t)
    }
}
