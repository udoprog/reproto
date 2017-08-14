use super::*;
use macros::FormatAttribute;
use pulldown_cmark as markdown;
use std::collections::HashMap;
use std::rc::Rc;

pub struct DocBackend {
    pub env: Environment,
    #[allow(dead_code)]
    options: DocOptions,
    listeners: Box<DocListeners>,
    pub theme: String,
    pub themes: HashMap<&'static str, &'static [u8]>,
}

include!(concat!(env!("OUT_DIR"), "/themes.rs"));

fn build_themes() -> HashMap<&'static str, &'static [u8]> {
    let mut m = HashMap::new();

    for (key, value) in build_themes_vec() {
        m.insert(key, value);
    }

    m
}

impl DocBackend {
    pub fn new(
        env: Environment,
        options: DocOptions,
        listeners: Box<DocListeners>,
        theme: String,
    ) -> DocBackend {
        DocBackend {
            env: env,
            options: options,
            listeners: listeners,
            theme: theme,
            themes: build_themes(),
        }
    }

    pub fn verify(&self) -> Result<()> {
        Ok(())
    }

    fn type_url(&self, pos: &Pos, lookup_id: &RpTypeId) -> Result<String> {
        let LookupResult {
            package,
            registered,
            type_id,
            ..
        } = self.env
            .lookup(&lookup_id.package, &lookup_id.name)
            .map_err(|e| Error::pos(e.description().to_owned(), pos.into()))?;

        let fragment = registered.local_name(&type_id, |p| p.join("_"), |c| c.join("_"));

        if let Some(_) = lookup_id.name.prefix {
            let package = self.package(package);
            let package = self.package_file(&package);
            return Ok(format!("{}.html#{}", package, fragment));
        }

        return Ok(format!("#{}", fragment));
    }

    fn markdown(input: &str) -> String {
        let p = markdown::Parser::new(input);
        let mut s = String::new();
        markdown::html::push_html(&mut s, p);
        s
    }

    pub fn package_file(&self, package: &RpPackage) -> String {
        package.parts.join("_")
    }

    fn write_markdown(&self, out: &mut DocBuilder, comment: &[String]) -> Result<()> {
        if !comment.is_empty() {
            let comment = comment.join("\n");
            write!(out, "{}", Self::markdown(&comment))?;
        }

        Ok(())
    }

    fn write_description<'a, I>(&self, out: &mut DocBuilder, comment: I) -> Result<()>
    where
        I: IntoIterator<Item = &'a String>,
    {
        let mut it = comment.into_iter().peekable();

        if it.peek().is_some() {
            let comment = it.map(ToOwned::to_owned).collect::<Vec<_>>();
            let comment = comment.join("\n");
            html!(out, div { class => "description" } ~ Self::markdown(&comment));
        }

        Ok(())
    }

    fn write_variants<'b, I>(&self, out: &mut DocBuilder, variants: I) -> Result<()>
    where
        I: IntoIterator<Item = &'b Loc<Rc<RpEnumVariant>>>,
    {
        let mut it = variants.into_iter().peekable();

        if it.peek().is_none() {
            return Ok(());
        }

        html!(out, div {class => "variants"} => {
            html!(out, h2 {} ~ "Variants");

            html!(out, table {class => "spaced"} => {
                for variant in it {
                    html!(out, tr {} => {
                        html!(out, td {class => "name"} ~ variant.name.as_ref());

                        html!(out, td {class => "description"} => {
                            self.write_description(out, &variant.comment)?;
                        });
                    });
                }
            });
        });

        Ok(())
    }

    fn write_simple_type(&self, out: &mut DocBuilder, name: &'static str) -> Result<()> {
        html!(out, span {class => format!("type-{}", name)} => {
            html!(out, code {class => "type-name"} ~ name);
        });

        Ok(())
    }

    fn write_type(
        &self,
        out: &mut DocBuilder,
        pos: &Pos,
        type_id: &RpTypeId,
        ty: &RpType,
    ) -> Result<()> {
        write!(out, "<span class=\"ty\">")?;

        match *ty {
            RpType::Double => self.write_simple_type(out, "double")?,
            RpType::Float => self.write_simple_type(out, "float")?,
            RpType::Boolean => self.write_simple_type(out, "boolean")?,
            RpType::String => self.write_simple_type(out, "string")?,
            RpType::Bytes => self.write_simple_type(out, "bytes")?,
            RpType::Any => self.write_simple_type(out, "any")?,
            RpType::Signed { ref size } => {
                html!(out, span {class => "type-signed"} => {
                    html!(out, code {class => "type-name"} ~ "signed");

                    if let Some(ref size) = *size {
                        html!(out, span {class => "type-size-sep"} ~ "/");
                        html!(out, span {class => "type-size"} ~ format!("{}", size));
                    }
                });
            }
            RpType::Unsigned { ref size } => {
                html!(out, span {class => "type-unsigned"} => {
                    html!(out, code {class => "type-name"} ~ "unsigned");

                    if let Some(ref size) = *size {
                        html!(out, span {class => "type-size-sep"} ~ "/");
                        html!(out, span {class => "type-size"} ~ format!("{}", size));
                    }
                });
            }
            RpType::Name { ref name } => {
                let url = self.type_url(pos, &type_id.with_name(name.clone()))?;
                let name = name.join("::");

                html!(out, span {class => "type-rp-name"} => {
                    html!(out, a {href => url} ~ name);
                });
            }
            RpType::Array { ref inner } => {
                html!(out, span {class => "type-array"} => {
                    html!(out, span {class => "type-array-left"} ~ "[");
                    self.write_type(out, pos, type_id, inner)?;
                    html!(out, span {class => "type-array-right"} ~ "]");
                });
            }
            RpType::Map { ref key, ref value } => {
                html!(out, span {class => "type-map"} => {
                    html!(out, span {class => "type-map-left"} ~ "{");
                    self.write_type(out, pos, type_id, key)?;
                    html!(out, span {class => "type-map-sep"} ~ ":");
                    self.write_type(out, pos, type_id, value)?;
                    html!(out, span {class => "type-map-right"} ~ "}");
                });
            }
        }

        write!(out, "</span>")?;
        Ok(())
    }

    fn write_fields<'b, I>(&self, out: &mut DocBuilder, type_id: &RpTypeId, fields: I) -> Result<()>
    where
        I: Iterator<Item = &'b Loc<RpField>>,
    {
        html!(out, div {class => "fields"} => {
            html!(out, h2 {} ~ "Fields");

            html!(out, table {class => "spaced"} => {
                for field in fields {
                    let (field, pos) = field.ref_both();

                    let mut classes = vec!["field"];

                    if field.is_optional() {
                        classes.push("optional");
                    } else {
                        classes.push("required");
                    }

                    html!(out, tr {classes => classes} => {
                        html!(out, td {class => "mime"} => {
                            let ident = field.ident();
                            let name = field.name();

                            html!(out, span {class => "field-ident"} ~ ident);

                            if field.is_optional() {
                                html!(out, span {class => "field-modifier"} ~ "?");
                            }

                            if name != ident {
                                html!(out, span {class => "field-alias"} => {
                                    html!(out, span {class => "field-alias-as"} ~ "as");
                                    html!(out, code {class => "field-alias-name"} ~ format!("\"{}\"", name));
                                });
                            }
                        });

                        html!(out, td {class => "type"} => {
                            self.write_type(out, pos, type_id, &field.ty)?;
                        });

                        html!(out, td {class => "description"} => {
                            self.write_markdown(out, &field.comment)?;
                        });
                    });
                }
            });
        });

        Ok(())
    }

    fn section_title(&self, out: &mut DocBuilder, ty: &str, name: &str, id: &str) -> Result<()> {
        html!(out, h1 {class => "section-title"} => {
            html!(out, a {class => "link", href => format!("#{}", id)} ~ Escape(name));
            html!(out, span {class => "type"} ~ ty);
        });

        Ok(())
    }

    pub fn write_doc<Body>(&self, out: &mut DocBuilder, body: Body) -> Result<()>
    where
        Body: FnOnce(&mut DocBuilder) -> Result<()>,
    {
        html!(out, html {} => {
            html!(out, head {} => {
                html!(@open out, meta {charset => "utf-8"});
                out.new_line()?;

                html!(@open out, meta {
                    name => "viewport",
                    content => "width=device-width, initial-scale=1.0"
                });
                out.new_line()?;

                html!(@open out, link {
                    rel => "stylesheet", type => "text/css", href => NORMALIZE_CSS_NAME
                });
                out.new_line()?;

                html!(@open out, link {
                    rel => "stylesheet", type => "text/css", href => DOC_CSS_NAME
                });
            });

            html!(out, body {} => {
                body(out)?;
            });
        });

        Ok(())
    }

    fn write_endpoint_short(
        &self,
        out: &mut DocBuilder,
        index: usize,
        body: &Rc<RpServiceBody>,
        endpoint: &RpServiceEndpoint,
    ) -> Result<()> {
        let method = endpoint.method().unwrap_or("GET").to_owned();
        let id = format!("{}_{}_{}", body.name, endpoint.id_parts(Self::fragment_filter).join("_"), index);

        html!(out, div {class => format!("endpoint short {}", method.to_lowercase())} => {
            html!(out, a {class => "endpoint-title", href => format!("#{}", id)} => {
                html!(out, span {class => "method"} ~ Escape(method.as_ref()));
                html!(out, span {class => "url"} ~ Escape(endpoint.url().as_ref()));
            });

            if !endpoint.comment.is_empty() {
                html!(out, div {class => "endpoint-body"} => {
                    self.write_description(out, endpoint.comment.iter().take(1))?;
                });
            }
        });

        Ok(())
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

    fn write_endpoint(
        &self,
        out: &mut DocBuilder,
        index: usize,
        type_id: &RpTypeId,
        body: &Rc<RpServiceBody>,
        endpoint: &RpServiceEndpoint,
    ) -> Result<()> {
        let method = endpoint.method().unwrap_or("GET").to_owned();
        let id = format!("{}_{}_{}", body.name, endpoint.id_parts(Self::fragment_filter).join("_"), index);

        html!(out, div {class => format!("endpoint {}", method.to_lowercase()), id => id} => {
            html!(out, h2 {class => "endpoint-title"} => {
                html!(out, span {class => "method"} ~ Escape(method.as_ref()));

                html!(out, a {class => "url", href => format!("#{}", id)}
                    ~ Escape(endpoint.url().as_ref()));
            });

            html!(out, div {class => "endpoint-body"} => {
                self.write_description(out, &endpoint.comment)?;

                if !endpoint.accepts.is_empty() {
                    html!(out, h2 {} ~ "Accepts");

                    html!(out, table {class => "spaced"} => {
                        for accept in &endpoint.accepts {
                            html!(out, tr {} => {
                                let accepts = accept.accepts
                                    .as_ref()
                                    .map(|m| format!("{}", m))
                                    .unwrap_or("*/*".to_owned());

                                html!(out, td {class => "mime"} => {
                                    html!(out, code {} ~ Escape(accepts.as_ref()))
                                });

                                html!(out, td {class => "type"} => {
                                    if let Some(ref ty) = accept.ty {
                                        let (ty, pos) = ty.ref_both();
                                        self.write_type(out, pos, type_id, ty)?;
                                    } else {
                                        html!(out, em {} ~ "no body");
                                    }
                                });

                                html!(out, td {class => "description"} => {
                                    self.write_markdown(out, &accept.comment)?;
                                });
                            });
                        }
                    });
                }

                if !endpoint.returns.is_empty() {
                    html!(out, h2 {} ~ "Returns");

                    html!(out, table {class => "spaced"} => {
                        for response in &endpoint.returns {
                            html!(out, tr {} => {
                                let status = response.status
                                    .as_ref()
                                    .map(|status| format!("{}", status))
                                    .unwrap_or("<em>no status</em>".to_owned());

                                let produces = response.produces
                                    .as_ref()
                                    .map(|m| format!("{}", m))
                                    .unwrap_or("*/*".to_owned());

                                html!(out, td {class => "status"} ~ status);
                                html!(out, td {class => "mime"} => {
                                    html!(out, code {} ~ Escape(produces.as_ref()))
                                });

                                html!(out, td {class => "type"} => {
                                    if let Some(ref ty) = response.ty {
                                        let (ty, pos) = ty.ref_both();
                                        self.write_type(out, pos, type_id, ty)?;
                                    } else {
                                        html!(out, em {} ~ "no body");
                                    }
                                });

                                html!(out, td {class => "description"} => {
                                    self.write_markdown(out, &response.comment)?;
                                });
                            });
                        }
                    });
                }
            });
        });

        Ok(())
    }

    /// Write a packages index.
    ///
    /// * `current` if some value indicates which the current package is.
    pub fn write_packages(
        &self,
        out: &mut DocBuilder,
        packages: &[RpVersionedPackage],
        current: Option<&RpVersionedPackage>,
    ) -> Result<()> {
        html!(out, section {class => "section-content section-packages"} => {
            html!(out, h1 {class => "section-title"} ~ "Packages");

            html!(out, div {class => "section-body"} => {
                html!(out, ul {class => "packages-list"} => {
                    for package in packages {
                        let name = format!("{}", package);

                        if let Some(current) = current {
                            if package == current {
                                html!(out, li {} ~ format!("<b>{}</b>", Escape(name.as_ref())));
                                continue;
                            }
                        }

                        let package = self.package(package);
                        let url = format!("{}.{}", self.package_file(&package), EXT);

                        html!(out, li {} => {
                            html!(out, a {href => url} ~ Escape(name.as_ref()));
                        });
                    }
                });
            });
        });

        Ok(())
    }

    pub fn write_service_overview(
        &self,
        out: &mut DocBuilder,
        service_bodies: Vec<Rc<RpServiceBody>>,
    ) -> Result<()> {
        if service_bodies.is_empty() {
            return Ok(());
        }

        html!(out, section {class => "section-content section-service-overview"} => {
            html!(out, h1 {class => "section-title"} ~ "Services");

            html!(out, div {class => "section-body"} => {
                for body in service_bodies {
                    html!(out, h2 {} ~ &body.name);

                    self.write_description(out, body.comment.iter().take(1))?;

                    for (index, endpoint) in body.endpoints.iter().enumerate() {
                        self.write_endpoint_short(out, index, &body, endpoint)?;
                    }
                }
            })
        });

        Ok(())
    }

    pub fn write_types_overview(&self, out: &mut DocBuilder, decls: Vec<RpDecl>) -> Result<()> {
        if decls.is_empty() {
            return Ok(());
        }

        html!(out, section {class => "section-content section-types-overview"} => {
            html!(out, h1 {class => "section-title"} ~ "Types");

            html!(out, div {class => "section-body"} => {
                for decl in decls {
                    let href = format!("#{}", decl.name());

                    html!(out, h2 {} => {
                        html!(out, a {href => href} ~ decl.name());
                    });

                    self.write_description(out, decl.comment().iter().take(1))?;
                }
            })
        });

        Ok(())
    }

    pub fn process_service(
        &self,
        out: &mut DocCollector,
        type_id: &RpTypeId,
        _: &Pos,
        body: Rc<RpServiceBody>,
    ) -> Result<()> {
        let mut new_service = out.new_service(body.clone());
        let mut out = DefaultDocBuilder::new(&mut new_service);

        let name = type_id.name.join("::");
        let id = type_id.name.join("_");

        html!(out, section {id => &id, class => "section-content section-service"} => {
            self.section_title(&mut out, "service", &name, &id)?;

            html!(out, div {class => "section-body"} => {
                self.write_description(&mut out, &body.comment)?;

                for (index, endpoint) in body.endpoints.iter().enumerate() {
                    self.write_endpoint(&mut out, index, type_id, &body, endpoint)?;
                }
            });
        });

        Ok(())
    }

    pub fn process_enum(
        &self,
        out: &mut DocCollector,
        type_id: &RpTypeId,
        _: &Pos,
        body: Rc<RpEnumBody>,
    ) -> Result<()> {
        let mut new_enum = out.new_type(RpDecl::Enum(body.clone()));
        let mut out = DefaultDocBuilder::new(&mut new_enum);

        let name = type_id.name.join("::");
        let id = type_id.name.join("_");

        html!(out, section {id => &id, class => "section-content section-enum"} => {
            self.section_title(&mut out, "enum", &name, &id)?;

            html!(out, div {class => "section-body"} => {
                self.write_description(&mut out, &body.comment)?;
                self.write_variants(&mut out, body.variants.iter())?;
            });
        });

        Ok(())
    }

    pub fn process_interface(
        &self,
        out: &mut DocCollector,
        type_id: &RpTypeId,
        _: &Pos,
        body: Rc<RpInterfaceBody>,
    ) -> Result<()> {
        let mut new_interface = out.new_type(RpDecl::Interface(body.clone()));
        let mut out = DefaultDocBuilder::new(&mut new_interface);

        let name = type_id.name.join("::");
        let id = type_id.name.join("_");

        html!(out, section {id => &id, class => "section-content section-interface"} => {
            self.section_title(&mut out, "interface", &name, &id)?;

            html!(out, div {class => "section-body"} => {
                self.write_description(&mut out, &body.comment)?;

                if !body.sub_types.is_empty() {
                    html!(out, div {class => "sub-types"} => {
                        for (name, sub_type) in &body.sub_types {
                            let id = format!("{}_{}", body.name, sub_type.name);

                            html!(out, h2 {id => id, class => "sub-type-title"} => {
                                html!(out, a {class => "link", href => format!("#{}", id)} ~ name);
                            });

                            self.write_description(&mut out, &body.comment)?;

                            let fields = body.fields.iter().chain(sub_type.fields.iter());
                            self.write_fields(&mut out, type_id, fields)?;
                        }
                    });
                }
            });
        });

        Ok(())
    }

    pub fn process_type(
        &self,
        out: &mut DocCollector,
        type_id: &RpTypeId,
        _: &Pos,
        body: Rc<RpTypeBody>,
    ) -> Result<()> {
        let mut new_type = out.new_type(RpDecl::Type(body.clone()));
        let mut out = DefaultDocBuilder::new(&mut new_type);

        let name = type_id.name.join("::");
        let id = type_id.name.join("_");

        html!(out, section {id => &id, class => "section-content section-type"} => {
            self.section_title(&mut out, "type", &name, &id)?;

            html!(out, div {class => "section-body"} => {
                self.write_description(&mut out, &body.comment)?;
                self.write_fields(&mut out, type_id, body.fields.iter())?;
            });
        });

        Ok(())
    }

    pub fn process_tuple(
        &self,
        out: &mut DocCollector,
        type_id: &RpTypeId,
        _: &Pos,
        body: Rc<RpTupleBody>,
    ) -> Result<()> {
        let mut new_tuple = out.new_type(RpDecl::Tuple(body.clone()));
        let mut out = DefaultDocBuilder::new(&mut new_tuple);

        let name = type_id.name.join("::");
        let id = type_id.name.join("_");

        html!(out, section {id => &id, class => "section-content section-tuple"} => {
            self.section_title(&mut out, "tuple", &name, &id)?;

            html!(out, div {class => "section-body"} => {
                self.write_description(&mut out, &body.comment)?;
                self.write_fields(&mut out, type_id, body.fields.iter())?;
            });
        });

        Ok(())
    }
}

impl PackageUtils for DocBackend {
    fn version_package(input: &Version) -> String {
        format!("{}", input).replace(Self::package_version_unsafe, "_")
    }
}
