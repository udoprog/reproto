//! gRPC module for Rust.

use backend::Initializer;
use core::errors::Result;
use core::{self, Loc};
use flavored::{RpEndpointHttp1, RustEndpoint};
use genco::rust::{imported, local};
use genco::{Cons, IntoTokens, Quoted, Rust, Tokens};
use utils::Comments;
use {Options, Service, ServiceCodegen};

pub struct Module {}

impl Module {
    pub fn new() -> Module {
        Module {}
    }
}

impl Initializer for Module {
    type Options = Options;

    fn initialize(&self, options: &mut Options) -> Result<()> {
        options.service.push(Box::new(ReqwestService::new()));

        Ok(())
    }
}

struct ReqwestService {
    client: Rust<'static>,
}

impl ReqwestService {
    pub fn new() -> Self {
        Self {
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
                t.nested({
                    let option = local("Option");

                    let mut t = Tokens::new();

                    let mut args = Tokens::new();
                    args.append(toks!["client: ", self.client.clone()]);

                    match body.http.url {
                        Some(_) => {
                            args.append(toks![
                                "url: ",
                                option.with_arguments(vec![url_ty.clone()]),
                            ]);
                        }
                        None => {
                            args.append(toks!["url: ", url_ty.clone()]);
                        }
                    };

                    let result = imported("reqwest", "Result");
                    let s = result.with_arguments(vec![local("Self")]);

                    push!(t, "pub fn new(", args.join(", "), ") -> ", s, " {");

                    t.nested({
                        let mut t = Tokens::new();

                        t.push_into(|t| match body.http.url {
                            Some(ref url) => {
                                let url = Loc::value(url).clone().quoted();

                                push!(t, "let url = match url {");
                                nested!(t, "Some(url) => url,");
                                nested!(t, "None => ", url_ty, "::parse(", url, ")?,");
                                push!(t, "};");
                            }
                            None => {
                                push!(t, "let url = Some(url);");
                            }
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
                        t.push(Endpoint { e, http });

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

struct Endpoint<'el> {
    e: &'el RustEndpoint,
    http: &'el RpEndpointHttp1,
}

impl<'el> IntoTokens<'el, Rust<'el>> for Endpoint<'el> {
    fn into_tokens(self) -> Tokens<'el, Rust<'el>> {
        let Endpoint { e, http } = self;

        let mut t = Tokens::new();

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

        let result = imported("reqwest", "Result");
        let encode = imported("reqwest::header::parsing", "http_percent_encode");

        let res = if let Some(ref res) = http.response {
            toks![result, "<", res, ">"]
        } else {
            toks![result, "<()>"]
        };

        push!(t, "pub fn ", e.safe_ident(), "(", args, ") -> ", res, " {");

        t.nested({
            let mut t = Tokens::new();

            let method = match http.method {
                core::RpHttpMethod::Get => "Get",
                core::RpHttpMethod::Post => "Post",
                core::RpHttpMethod::Put => "Put",
                core::RpHttpMethod::Update => "Update",
                core::RpHttpMethod::Delete => "Delete",
                core::RpHttpMethod::Patch => "Patch",
                core::RpHttpMethod::Head => "Head",
            };

            let m = toks![imported("reqwest", "Method"), "::", method];

            if let Some(ref http_path) = e.http.path {
                push!(t, "let mut path_ = String::new();");

                for step in &http_path.steps {
                    for part in &step.parts {
                        push!(t, "path_.push_str(", "/".quoted(), ");");

                        match *part {
                            core::RpPathPart::Variable(ref arg) => {
                                let var = toks![arg.safe_ident(), ".to_string().as_bytes()"];
                                push!(t, encode, "(&mut path_, ", var, ")?;");
                            }
                            core::RpPathPart::Segment(ref s) => {
                                push!(t, "path_.push_str(", s.as_str().quoted(), ");");
                            }
                        }
                    }
                }

                push!(t, "let url_ = self.url.join(&path_)?;");
            } else {
                push!(t, "let url_ = self.url.clone();");
            }

            let req = toks!["self.client.request(", m, ", url_)"];

            push!(t, "let req_ = ", req, ";");

            if let Some(ref req) = e.request {
                push!(t, "let req_ = req_.json(&", req.safe_ident(), ");");
            }

            if e.response.is_some() {
                push!(t, "let res_ = req_.send()?;");
                push!(t, "res_.json()");
            } else {
                push!(t, "req_.send()?;");
                push!(t, "Ok(())");
            }

            t
        });

        push!(t, "}");

        t
    }
}
