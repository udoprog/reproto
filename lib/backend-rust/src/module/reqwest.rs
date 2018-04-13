//! gRPC module for Rust.

use backend::Initializer;
use core::errors::Result;
use core::{self, Loc};
use flavored::{RpEndpointHttp1, RpPackage, RpPathSpec, RpServiceBody, RustEndpoint};
use genco::rust::{imported, local};
use genco::{Cons, IntoTokens, Quoted, Rust, Tokens};
use std::rc::Rc;
use utils::Comments;
use {Options, Root, RootCodegen, RustFileSpec, Service, ServiceCodegen, SCOPE_SEP};

pub struct Module {}

impl Module {
    pub fn new() -> Module {
        Module {}
    }
}

impl Initializer for Module {
    type Options = Options;

    fn initialize(&self, options: &mut Options) -> Result<()> {
        let utils_package = options.packages.new("reproto")?;

        let imported_utils_package = Rc::new(utils_package.join(SCOPE_SEP));
        let result = imported(imported_utils_package.clone(), "Result");
        let path_encode = imported(imported_utils_package.clone(), "PathEncode");

        options
            .service
            .push(Box::new(ReqwestService::new(result, path_encode)));

        options
            .root
            .push(Box::new(ReqwestUtils::new(utils_package)));

        Ok(())
    }
}

struct ReqwestUtils {
    utils_package: RpPackage,
}

impl ReqwestUtils {
    pub fn new(utils_package: RpPackage) -> Self {
        Self { utils_package }
    }

    fn reproto<'el>(&self) -> Result<RustFileSpec<'el>> {
        let mut f = RustFileSpec::default();

        let mut errors = Vec::new();
        errors.push((imported("reqwest", "Error"), "ReqwestError"));
        errors.push((imported("reqwest", "UrlError"), "UrlError"));
        errors.push((imported("std::fmt", "Error"), "FormatError"));

        f.0.push({
            let mut t = Tokens::new();

            push!(t, "#[derive(Debug)]");
            push!(t, "pub enum Error {");

            for &(ref ty, ref variant) in &errors {
                nested!(t, variant.clone(), "(", ty.clone(), "),");
            }

            push!(t, "}");

            t
        });

        f.0.push({
            let mut t = Tokens::new();
            let result = imported("std::result", "Result");

            push!(t, "pub type Result<T> = ", result, "<T, Error>;");

            t
        });

        for &(ref ty, ref variant) in &errors {
            f.0.push({
                let mut t = Tokens::new();

                push!(t, "impl From<", ty.clone(), "> for Error {");

                t.nested({
                    let mut t = Tokens::new();

                    push!(t, "fn from(value: ", ty.clone(), ") -> Self {");
                    nested!(t, "Error::", variant.clone(), "(value)");
                    push!(t, "}");

                    t
                });

                push!(t, "}");

                t
            });
        }

        // fmt::Display implementation for Error
        f.0.push({
            let mut t = Tokens::new();

            let display = imported("std::fmt", "Display");
            let formatter = imported("std::fmt", "Formatter");
            let result = imported("std::fmt", "Result");

            push!(t, "impl ", display, " for Error {");

            t.nested_into(|t| {
                push!(
                    t,
                    "fn fmt(&self, fmt: &mut ",
                    formatter,
                    ") -> ",
                    result,
                    " {"
                );

                t.nested_into(|t| {
                    push!(t, "match *self {");

                    for &(_, ref variant) in &errors {
                        nested!(t, "Error::", variant.clone(), "(ref e) => e.fmt(fmt),");
                    }

                    push!(t, "}");
                });

                push!(t, "}");
            });

            push!(t, "}");

            t
        });

        f.0.push({
            let mut t = Tokens::new();

            let display = imported("std::fmt", "Display");
            let fmt = imported("std::fmt", "Formatter");
            let result = imported("std::fmt", "Result");
            let encode = imported("reqwest::header::parsing", "http_percent_encode");

            push!(t, "pub struct PathEncode<T>(pub T);");

            t.push({
                let mut t = Tokens::new();

                push!(t, "impl<T> ", display, " for PathEncode<T>");
                push!(t, "where");
                nested!(t, "T: ", display);
                push!(t, "{");

                t.nested({
                    let mut t = Tokens::new();

                    push!(t, "fn fmt(&self, fmt: &mut ", fmt, ") -> ", result, " {");
                    nested!(t, encode, "(fmt, self.0.to_string().as_bytes())");
                    push!(t, "}");

                    t
                });

                push!(t, "}");

                t
            });

            t.join_line_spacing()
        });

        Ok(f)
    }
}

impl RootCodegen for ReqwestUtils {
    fn generate(&self, root: Root) -> Result<()> {
        let Root { files, .. } = root;
        files.insert(self.utils_package.clone(), self.reproto()?);
        Ok(())
    }
}

struct ReqwestService {
    result: Rust<'static>,
    path_encode: Rust<'static>,
    client: Rust<'static>,
}

impl ReqwestService {
    pub fn new(result: Rust<'static>, path_encode: Rust<'static>) -> Self {
        Self {
            result,
            path_encode,
            client: imported("reqwest", "Client"),
        }
    }
}

impl ServiceCodegen for ReqwestService {
    fn generate(&self, service: Service) -> Result<()> {
        let Service {
            body,
            container,
            name,
            attributes,
            ..
        } = service;

        let name = Cons::from(format!("{}_Reqwest", name));
        let url_ty = imported("reqwest", "Url");

        container.push({
            let mut t = Tokens::new();

            t.push_unless_empty(attributes.clone());
            push!(t, "pub struct ", name, "{");
            nested!(t, "client: ", self.client, ",");
            nested!(t, "url: ", url_ty, ",");
            push!(t, "}");

            t
        });

        container.push({
            let mut t = Tokens::new();

            push!(t, "impl ", name, " {");

            t.push_unless_empty({
                let mut t = Tokens::new();

                // constructor.
                t.nested(Constructor {
                    result: &self.result,
                    client: &self.client,
                    body,
                    url_ty: &url_ty,
                });

                // endpoint methods.
                for e in &body.endpoints {
                    let http = match e.http1.as_ref() {
                        Some(http) => http,
                        None => continue,
                    };

                    t.nested({
                        let mut t = Tokens::new();

                        t.push_unless_empty(Comments(&e.comment));
                        t.push(Endpoint {
                            result: &self.result,
                            path_encode: &self.path_encode,
                            e,
                            http,
                        });

                        t
                    });
                }

                t.join_line_spacing()
            });

            push!(t, "}");

            t
        });

        Ok(())
    }
}

/// Builds a constructor for the service struct.
struct Constructor<'a, 'el: 'a> {
    body: &'el RpServiceBody,
    result: &'a Rust<'static>,
    client: &'a Rust<'static>,
    url_ty: &'a Rust<'static>,
}

impl<'a, 'el: 'a> IntoTokens<'el, Rust<'el>> for Constructor<'a, 'el> {
    fn into_tokens(self) -> Tokens<'el, Rust<'el>> {
        let Constructor {
            body,
            result,
            client,
            url_ty,
            ..
        } = self;

        let option = local("Option");

        let mut t = Tokens::new();

        let mut args = Tokens::new();
        args.append(toks!["client: ", client.clone()]);

        match body.http.url {
            Some(_) => {
                args.append(toks!["url: ", option.with_arguments(vec![url_ty.clone()]),]);
            }
            None => {
                args.append(toks!["url: ", url_ty.clone()]);
            }
        };

        let s = result.clone().with_arguments(vec![local("Self")]);

        push!(t, "pub fn new(", args.join(", "), ") -> ", s, " {");

        t.nested({
            let mut t = Tokens::new();

            t.push_into(|t| match body.http.url {
                Some(ref url) => {
                    let url = Loc::borrow(url).clone().quoted();

                    push!(t, "let url = match url {");
                    nested!(t, "Some(url) => url,");
                    nested!(t, "None => ", url_ty.clone(), "::parse(", url, ")?,");
                    push!(t, "};");
                }
                None => {}
            });

            t.push_into(|t| {
                push!(t, "Ok(Self {");
                nested!(t, "client,");
                nested!(t, "url,");
                push!(t, "})");
            });

            t.join_line_spacing()
        });

        push!(t, "}");

        t
    }
}

/// Write full path to a string.
struct WritePath<'a, 'el: 'a> {
    var: &'el str,
    path: &'el RpPathSpec,
    path_encode: &'a Rust<'el>,
}

impl<'a, 'el: 'a> IntoTokens<'el, Rust<'el>> for WritePath<'a, 'el> {
    fn into_tokens(self) -> Tokens<'el, Rust<'el>> {
        let WritePath {
            var,
            path,
            path_encode,
        } = self;

        let mut t = Tokens::new();

        for step in &path.steps {
            push!(t, var, ".push_str(", "/".quoted(), ");");

            for part in &step.parts {
                match *part {
                    core::RpPathPart::Variable(ref arg) => {
                        let expr = toks![path_encode.clone(), "(", arg.safe_ident(), ")"];
                        push!(t, "write!(", var, ", ", "{}".quoted(), ", ", expr, ")?;");
                    }
                    core::RpPathPart::Segment(ref s) => {
                        push!(t, var, ".push_str(", s.as_str().quoted(), ");");
                    }
                }
            }
        }

        t
    }
}

/// Build an endpoint method for the service struct.
struct Endpoint<'a, 'el: 'a> {
    result: &'a Rust<'static>,
    path_encode: &'a Rust<'static>,
    e: &'el RustEndpoint,
    http: &'el RpEndpointHttp1,
}

impl<'a, 'el: 'a> IntoTokens<'el, Rust<'el>> for Endpoint<'a, 'el> {
    fn into_tokens(self) -> Tokens<'el, Rust<'el>> {
        use core::RpHttpMethod::*;

        let Endpoint {
            result,
            path_encode,
            e,
            http,
        } = self;

        let mut t = Tokens::new();

        // import trait
        t.register(imported("std::fmt", "Write").qualified());

        let mut args = Tokens::new();

        args.append("&self");

        for a in &e.arguments {
            args.append({
                let mut t = Tokens::new();

                t.append(a.safe_ident());
                t.append(": ");
                t.append(a.channel.ty());

                t
            });
        }

        let args = args.join(", ");

        let res = if let Some(ref res) = http.response {
            toks![result.clone(), "<", res, ">"]
        } else {
            toks![result.clone(), "<()>"]
        };

        push!(t, "pub fn ", e.safe_ident(), "(", args, ") -> ", res, " {");

        t.nested({
            let mut t = Tokens::new();

            if let Some(ref path) = e.http.path {
                let mut p = Tokens::new();

                push!(p, "let mut path_ = String::new();");

                p.push(WritePath {
                    var: "path_",
                    path,
                    path_encode,
                });

                t.push(p);
                push!(t, "let url_ = self.url.join(&path_)?;");
            } else {
                push!(t, "let url_ = self.url.clone();");
            }

            let method = match http.method {
                Get => "Get",
                Post => "Post",
                Put => "Put",
                Update => "Update",
                Delete => "Delete",
                Patch => "Patch",
                Head => "Head",
            };

            let m = toks![imported("reqwest", "Method"), "::", method];

            let req = toks!["self.client.request(", m, ", url_)"];

            push!(t, "let mut req_ = ", req, ";");

            if let Some(ref req) = e.request {
                push!(t, "req_.json(&", req.safe_ident(), ");");
            }

            if e.response.is_some() {
                push!(t, "let mut res_ = req_.send()?;");
                push!(t, "let body_ = res_.json()?;");
                push!(t, "Ok(body_)");
            } else {
                push!(t, "req_.send()?;");
                push!(t, "Ok(())");
            }

            t.join_line_spacing()
        });

        push!(t, "}");

        t
    }
}
