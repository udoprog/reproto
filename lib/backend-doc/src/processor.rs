//! Processor trait.

use super::{DOC_CSS_NAME, NORMALIZE_CSS_NAME};
use core::errors::*;
use core::flavored::{RpDecl, RpField, RpName, RpType, RpVersionedPackage};
use core::{self, CoreFlavor, ForEachLoc, Loc, WithPos};
use doc_builder::DocBuilder;
use escape::Escape;
use macros::FormatAttribute;
use rendering::markdown_to_html;
use std::ops::DerefMut;
use syntect::highlighting::Theme;
use syntect::parsing::SyntaxSet;
use trans::Translated;

pub trait Processor<'env> {
    /// Access the current builder.
    fn out(&self) -> ::std::cell::RefMut<DocBuilder<'env>>;

    /// Access the current environment.
    fn env(&self) -> &'env Translated<CoreFlavor>;

    /// Path to root.
    fn root(&self) -> &'env str;

    /// Process the given request.
    fn process(self) -> Result<()>;

    /// Syntax theme.
    fn syntax(&self) -> (&'env Theme, &'env SyntaxSet);

    fn current_package(&self) -> Option<&'env RpVersionedPackage> {
        None
    }

    /// Generate a type URL.
    fn type_url(&self, name: &RpName) -> Result<String> {
        let reg = self.env().lookup(name)?;

        let (fragment, parts) = match *reg {
            core::RpReg::EnumVariant | core::RpReg::SubType => {
                let fragment = format!("#{}", name.parts.clone().join("_"));

                let parts: Vec<_> = name.parts
                    .iter()
                    .cloned()
                    .take(name.parts.len() - 1)
                    .collect();

                (fragment, parts)
            }
            _ => {
                let fragment = "".to_string();
                (fragment, name.parts.clone())
            }
        };

        if let Some(_) = name.prefix {
            let path = name.package.as_package(|v| v.to_string()).parts.join("/");

            return Ok(format!(
                "{}/{}/{}.{}.html{}",
                self.root(),
                path,
                reg,
                parts.join("."),
                fragment,
            ));
        }

        Ok(format!("{}.{}.html{}", reg, parts.join("."), fragment))
    }

    fn markdown(&self, comment: &str) -> Result<()> {
        if !comment.is_empty() {
            let (theme, syntax_set) = self.syntax();
            markdown_to_html(self.out().deref_mut(), comment, theme, syntax_set)?;
        }

        Ok(())
    }

    fn doc<'a, I>(&self, comment: I) -> Result<()>
    where
        I: IntoIterator<Item = &'a String>,
    {
        let mut it = comment.into_iter().peekable();

        if it.peek().is_some() {
            let comment = it.map(ToOwned::to_owned).collect::<Vec<_>>();
            let comment = comment.join("\n");
            html!(self, div { class => "doc" } => {
                self.markdown(comment.as_str())?;
            });
        } else {
            html!(self, div { class => "missing-doc" } ~ Escape("no documentation :("));
        }

        Ok(())
    }

    fn primitive(&self, name: &str) -> Result<()> {
        html!(self, span {class => format!("type-{} type-primitive", name)} ~ name);
        Ok(())
    }

    fn write_type(&self, ty: &RpType) -> Result<()> {
        use core::RpType::*;

        write!(self.out(), "<span class=\"ty\">")?;

        match *ty {
            Double => self.primitive("double")?,
            Float => self.primitive("float")?,
            Boolean => self.primitive("boolean")?,
            String => self.primitive("string")?,
            DateTime => self.primitive("datetime")?,
            Bytes => self.primitive("bytes")?,
            Any => self.primitive("any")?,
            Signed { ref size } => self.primitive(format!("i{}", size).as_str())?,
            Unsigned { ref size } => self.primitive(format!("u{}", size).as_str())?,
            Name { ref name } => {
                html!(self, span {class => "type-rp-name"} => {
                    self.full_name_without_package(name)?;
                });
            }
            Array { ref inner } => {
                html!(self, span {class => "type-array"} => {
                    html!(self, span {class => "type-array-left"} ~ "[");
                    self.write_type(inner)?;
                    html!(self, span {class => "type-array-right"} ~ "]");
                });
            }
            Map { ref key, ref value } => {
                html!(self, span {class => "type-map"} => {
                    html!(self, span {class => "type-map-left"} ~ "{");
                    self.write_type(key)?;
                    html!(self, span {class => "type-map-sep"} ~ ":");
                    self.write_type(value)?;
                    html!(self, span {class => "type-map-right"} ~ "}");
                });
            }
        }

        write!(self.out(), "</span>")?;
        Ok(())
    }

    fn field(&self, field: &RpField) -> Result<()> {
        let mut classes = vec!["field"];

        if field.is_optional() {
            classes.push("optional");
        } else {
            classes.push("required");
        }

        html!(self, h2 {class => "field-title"} => {
            html!(self, span {class => "kind"} ~ "field");

            html!(self, span {class => "field-key"} => {
                html!(self, span {class => "field-id"} ~ Escape(field.ident()));

                if field.is_optional() {
                    html!(self, span {class => "field-modifier"} ~ "?");
                }

                html!(self, span {} ~ ":");
            });

            self.write_type(&field.ty)?;

            if field.ident() != field.name() {
                html!(self, span {class => "keyword"} ~ "as");
                html!(self, span {class => "field-name"} ~ Escape(field.name()));
            }
        });

        self.doc(&field.comment)?;

        Ok(())
    }

    fn fields<'b, I>(&self, fields: I) -> Result<()>
    where
        I: Iterator<Item = &'b Loc<RpField>>,
    {
        fields.for_each_loc(|field| self.field(field))?;
        Ok(())
    }

    /// Render a nested declaration
    fn nested_decl(&self, decl: &RpDecl) -> Result<()> {
        html!(self, h2 {class => "decl-title"} => {
            html!(self, span {class => "kind"} ~ format!("nested {}", decl.kind()));
            self.full_name_without_package(&decl.name())?;
        });

        self.doc(decl.comment().iter().take(1))?;
        Ok(())
    }

    /// Render a set of nested declarations
    fn nested_decls<'b, I>(&self, decls: I) -> Result<()>
    where
        I: Iterator<Item = &'b RpDecl>,
    {
        for decl in decls {
            self.nested_decl(decl).with_pos(decl.pos())?;
        }

        Ok(())
    }

    /// Write a section title.
    fn section_title(&self, kind: &str, name: &RpName) -> Result<()> {
        html!(self, h1 {class => "section-title"} => {
            html!(self, span {class => "kind"} ~ kind);
            self.full_name(name, Some(name))?;
        });

        Ok(())
    }

    /// Write a complete HTML document.
    fn write_doc<Body>(&self, body: Body) -> Result<()>
    where
        Body: FnOnce() -> Result<()>,
    {
        html!(self, html {} => {
            html!(self, head {} => {
                html!(@open self, meta {charset => "utf-8"});
                self.out().new_line()?;

                html!(@open self, meta {
                    name => "viewport",
                    content => "width=device-width, initial-scale=1.0"
                });
                self.out().new_line()?;

                html!(@open self, link {
                    rel => "stylesheet", type => "text/css",
                    href => format!("{}/{}", self.root(), NORMALIZE_CSS_NAME)
                });
                self.out().new_line()?;

                html!(@open self, link {
                    rel => "stylesheet", type => "text/css",
                    href => format!("{}/{}", self.root(), DOC_CSS_NAME)
                });
            });

            html!(self, body {} => {
                html!(self, div {class => "container"} => {
                    html!(self, nav {class => "top"} => {
                        html!(self, a {href => format!("{}/index.html", self.root())} ~ "Index");

                        if let Some(package) = self.current_package() {
                            let package_url = self.package_url(package);
                            html!(self, span {} ~ "&mdash;");
                            html!(self, a {href => package_url} ~ format!("Package: {}", package));
                        }
                    });

                    body()?;
                });
            });
        });

        Ok(())
    }

    fn package_url(&self, package: &RpVersionedPackage) -> String {
        let url = package
            .clone()
            .as_package(ToString::to_string)
            .parts
            .join("/");

        format!("{}/{}/index.html", self.root(), url)
    }

    fn fragment_filter(url: &str) -> String {
        let mut bytes = [0u8; 4];
        let mut buffer = String::with_capacity(url.len());

        for c in url.chars() {
            let encode = match c {
                'a'...'z' | 'A'...'Z' | '0'...'9' => false,
                '!' | '$' | '&' | '\'' | '(' | ')' | '*' | '+' | ',' | ';' => false,
                '-' | '.' | '_' | '~' | ':' | '@' | '/' | '?' => false,
                _ => true,
            };

            if encode {
                let result = c.encode_utf8(&mut bytes);

                for b in result.bytes() {
                    buffer.extend(format!("%{:X}", b).chars());
                }

                continue;
            }

            buffer.push(c);
        }

        buffer
    }

    /// Write the full path to a name.
    ///
    /// # Examples
    ///
    /// ```html
    /// <span class="name-part">Foo</span>
    /// <span class="name-sep">::</span>
    /// <span class="name-local">Bar</span>
    /// ```
    fn full_name(&self, name: &RpName, current: Option<&RpName>) -> Result<()> {
        /*let package_url = self.package_url(&name.package);
        html!(self, a {class => "name-package", href => package_url} ~ name.package.to_string());
        html!(self, span {class => "name-sep"} ~ "::");*/

        let mut it = name.parts.iter();
        let local = it.next_back().ok_or_else(|| "local part of name required")?;

        let mut parts = Vec::new();

        for part in it {
            parts.push(part.clone());
            let name = name.clone().with_parts(parts.clone());

            if Some(&name) == current {
                html!(self, span {class => "name-part"} ~ part);
            } else {
                let url = self.type_url(&name)?;
                html!(self, a {class => "name-part", href => url} ~ part);
            }

            html!(self, span {class => "name-sep"} ~ "::");
        }

        let url = self.type_url(name)?;
        html!(self, a {class => "name-local", href => url} ~ local);
        Ok(())
    }

    /// Local name fully linked.
    fn full_name_without_package(&self, name: &RpName) -> Result<()> {
        let mut it = name.parts.iter();
        let local = it.next_back().ok_or_else(|| "local part of name required")?;

        let mut parts = Vec::new();

        if let Some(ref prefix) = name.prefix {
            let package_url = self.package_url(&name.package);
            html!(self, a {class => "name-package", href => package_url} ~ prefix);
            html!(self, span {class => "name-sep"} ~ "::");
        }

        for part in it {
            parts.push(part.clone());
            let name = name.clone().with_parts(parts.clone());
            let url = self.type_url(&name)?;
            html!(self, a {class => "name-part", href => url} ~ part);
            html!(self, span {class => "name-sep"} ~ "::");
        }

        let url = self.type_url(name)?;
        html!(self, a {class => "name-local", href => url} ~ local);
        Ok(())
    }

    /// Write the name, but without a local part.
    fn name_until(&self, name: &RpName) -> Result<()> {
        for part in &name.parts {
            html!(self, span {class => "name-part"} ~ part);
            html!(self, span {class => "name-sep"} ~ "::");
        }

        Ok(())
    }
}
