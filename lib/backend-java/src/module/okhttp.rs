//! Module that adds fasterxml annotations to generated classes.

use codegen::{Configure, EndpointExtra, ServiceAdded, ServiceCodegen};
use core::{RpEndpoint, RpPathPart, RpPathStep};
use core::errors::*;
use genco::{Cons, IntoTokens, Java, Quoted, Tokens};
use genco::java::{imported, local, Argument, Class, Constructor, Field, Method, Modifier};
use utils::Override;

#[derive(Debug, Deserialize)]
pub enum Version {
    #[serde(rename = "1")]
    Version1,
    #[serde(rename = "2")]
    Version2,
}

impl Default for Version {
    fn default() -> Self {
        Version::Version1
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    version: Version,
}

pub struct Module {
    #[allow(dead_code)]
    config: Config,
}

impl Module {
    pub fn new(config: Config) -> Module {
        Module { config: config }
    }
}

impl Module {
    pub fn initialize(self, e: Configure) {
        e.options
            .service_generators
            .push(Box::new(OkHttpServiceCodegen::new()));
    }
}

/// Model for a nested `Builder` class.
pub struct Builder<'el> {
    ty: Java<'el>,
    optional: Java<'el>,
    client: Field<'el>,
    base_url: Field<'el>,
}

impl<'el> Builder<'el> {
    fn build(&self) -> Method<'el> {
        let mut args = Tokens::new();

        args.append(self.client.var());
        args.append(self.base_url.var());

        let mut build = Method::new("build");

        build.returns = self.ty.clone();

        push!(
            build.body,
            "final ",
            self.base_url.ty().as_value(),
            " ",
            self.base_url.var(),
            " = ",
            required(&self.base_url),
            ";"
        );
        push!(
            build.body,
            "return new ",
            self.ty,
            "(",
            args.join(", "),
            ");"
        );

        return build;

        fn required<'el>(field: &Field<'el>) -> Tokens<'el, Java<'el>> {
            let exc = imported("java.lang", "RuntimeException");
            let msg = format!("{}: is a required field", field.var());
            let exc = toks![exc, "(", msg.quoted(), ")"];

            toks!["this.", field.var(), ".orElseThrow(() -> new ", exc, ")"]
        }
    }

    fn field<N: Into<Cons<'el>>>(&self, name: N, ty: Java<'el>) -> Field<'el> {
        let mut f = Field::new(self.optional.with_arguments(vec![ty.clone()]), name);
        f.initializer(toks![self.optional.clone(), ".empty()"]);
        f.modifiers = vec![Modifier::Private];
        f
    }

    fn setter<N: Into<Cons<'el>>>(&self, name: N, ty: Java<'el>) -> Method<'el> {
        let name = name.into();
        let arg = Argument::new(ty.as_value(), name.clone());

        let mut f = Method::new(name.clone());

        push!(
            f.body,
            "this.",
            name,
            " = ",
            self.optional,
            ".of(",
            arg.var(),
            ");"
        );
        push!(f.body, "return this;");

        f.returns = local("OkHttpBuilder");
        f.arguments.push(arg);

        f
    }
}

impl<'el> IntoTokens<'el, Java<'el>> for Builder<'el> {
    fn into_tokens(self) -> Tokens<'el, Java<'el>> {
        let mut builder = Class::new("OkHttpBuilder");
        builder.modifiers = vec![Modifier::Static, Modifier::Public];

        let base_url = self.field(self.base_url.var(), self.base_url.ty());

        builder.fields.push(base_url.clone());
        builder
            .methods
            .push(self.setter(self.base_url.var(), self.base_url.ty()));

        let mut c = Constructor::new();

        c.arguments
            .push(Argument::new(self.client.ty(), self.client.var()));

        push!(
            c.body,
            "this.",
            self.client.var(),
            " = ",
            self.client.var(),
            ";"
        );

        builder.constructors.push(c);

        builder.fields.push(self.client.clone());

        builder.body.push(self.build());
        builder.into_tokens()
    }
}

pub struct OkHttpServiceCodegen {
    client: Java<'static>,
    request: Java<'static>,
    http_url: Java<'static>,
    callback: Java<'static>,
    call: Java<'static>,
    response: Java<'static>,
    io_exc: Java<'static>,
}

impl OkHttpServiceCodegen {
    pub fn new() -> Self {
        Self {
            client: imported("okhttp3", "OkHttpClient"),
            request: imported("okhttp3", "Request"),
            http_url: imported("okhttp3", "HttpUrl"),
            callback: imported("okhttp3", "Callback"),
            call: imported("okhttp3", "Call"),
            response: imported("okhttp3", "Response"),
            io_exc: imported("java.io", "IOException"),
        }
    }
}

impl OkHttpServiceCodegen {
    fn request<'el>(
        &self,
        mut method: Method<'el>,
        endpoint: &'el RpEndpoint,
        extra: &EndpointExtra<'el>,
        client: Field<'el>,
        base_url: Field<'el>,
    ) -> Result<Method<'el>> {
        let EndpointExtra { ref arguments, .. } = *extra;

        method.body.push({
            let mut t = Tokens::new();

            t.push(toks![
                "final ",
                self.http_url.clone(),
                " url_ = this.",
                base_url.var(),
                ".newBuilder()",
            ]);

            if let Some(ref path) = endpoint.http.path {
                for step in &path.steps {
                    let args = step_args(step, arguments)?;
                    t.nested(toks![".addPathSegment(", args.join(" + "), ")"]);
                }
            }

            t.nested(".build();");

            t
        });

        method.body.push({
            let mut t = Tokens::new();

            push!(
                t,
                "final ",
                self.request,
                " req_ = new ",
                self.request,
                ".Builder()"
            );

            let method = endpoint
                .http
                .method
                .as_ref()
                .map(|m| m.as_str())
                .unwrap_or("GET");

            nested!(t, ".url(url_)");
            // TODO: actually provide the body
            nested!(t, ".method(", method.quoted(), ", null)");
            nested!(t, ".build();");

            t
        });

        push!(
            method.body,
            "final ",
            method.returns,
            " future_ = new ",
            method.returns,
            "();"
        );

        method.body.push({
            let mut t = Tokens::new();

            let new_call = toks!["this.", client.var(), ".newCall(req_)"];

            push!(t, new_call, ".enqueue(new ", self.callback, "() {");

            t.nested({
                let mut t = Tokens::new();

                t.push({
                    let e = Argument::new(self.io_exc.clone(), "e");

                    let mut m = Method::new("onFailure");
                    m.annotation(Override);
                    m.arguments.push(Argument::new(self.call.clone(), "call"));
                    m.arguments.push(e.clone());

                    push!(m.body, "future_.completeExceptionally(", e.var(), ");");

                    m
                });

                t.push({
                    let response = Argument::new(self.response.clone(), "response");

                    let mut m = Method::new("onResponse");
                    m.annotation(Override);
                    m.arguments.push(Argument::new(self.call.clone(), "call"));
                    m.arguments.push(response.clone());

                    // push!(m.body, "future.fail(", e.var(), ");");

                    m.body.push_into(|t| {
                        let exc = toks![
                            "new IOException(",
                            "bad response: ".quoted(),
                            " + response)"
                        ];

                        t.push("if (!response.isSuccessful()) {");
                        nested!(t, "future_.completeExceptionally(", exc, ");");
                        t.push("} else {");
                        // TODO: complete with the response
                        nested!(t, "future_.complete(null);");
                        t.push("}");
                    });

                    m
                });

                t.join_line_spacing()
            });

            push!(t, "});");

            t
        });

        method.body.push("return future_;");

        method.body = method.body.join_line_spacing();
        return Ok(method);

        fn step_args<'el>(
            step: &'el RpPathStep,
            arguments: &[Argument<'el>],
        ) -> Result<Tokens<'el, Java<'el>>> {
            let mut args = Tokens::new();

            for part in &step.parts {
                match *part {
                    RpPathPart::Variable(ref arg) => {
                        let ty = arg.channel.ty();

                        if ty.is_primitive() {
                            args.append(toks![ty.as_boxed(), ".toString(", arg.safe_ident(), ")"]);
                        } else {
                            args.append(arg.safe_ident());
                        }
                    }
                    RpPathPart::Segment(ref s) => {
                        args.append(s.to_string().quoted());
                    }
                }
            }

            Ok(args)
        }
    }
}

impl ServiceCodegen for OkHttpServiceCodegen {
    fn generate(&self, e: ServiceAdded) -> Result<()> {
        let ServiceAdded {
            compiler,
            body,
            spec,
            extra,
            ..
        } = e;

        let mut c = Class::new("OkHttp");
        c.implements = vec![local(spec.name())];

        let client = Field::new(self.client.clone(), "client");
        let base_url = Field::new(self.http_url.clone(), "baseUrl");

        for (endpoint, extra) in body.endpoints.iter().zip(extra.iter()) {
            let EndpointExtra {
                ref name,
                ref response_ty,
                ref arguments,
                ..
            } = *extra;

            if !endpoint.has_http_support() {
                let mut m = Method::new(name.clone());
                m.annotation(Override);
                m.returns = response_ty.clone();
                m.arguments.extend(arguments.iter().cloned());

                // FIXME: compile-time error?
                m.body
                    .push("throw new RuntimeException(\"endpoint does not support HTTP\");");

                c.methods.push(m);
                continue;
            }

            let mut m = Method::new(name.clone());
            m.annotation(Override);
            m.returns = response_ty.clone();
            m.arguments.extend(arguments.iter().cloned());

            c.methods
                .push(self.request(m, endpoint, extra, client.clone(), base_url.clone())?);
        }

        c.constructors.push({
            let mut c = Constructor::new();
            let client_arg = Argument::new(client.ty(), client.var());

            push!(c.body, "this.", client.var(), " = ", client_arg.var(), ";");
            push!(c.body, "this.", base_url.var(), " = ", base_url.var(), ";");

            c.arguments.push(client_arg);
            c.arguments
                .push(Argument::new(base_url.ty(), base_url.var()));
            c
        });

        let builder = Builder {
            ty: local(c.name()),
            optional: compiler.optional.clone(),
            client: client.clone(),
            base_url: base_url.clone(),
        };

        c.fields.push(client);
        c.fields.push(base_url);

        spec.body.push(c);
        spec.body.push(builder);
        Ok(())
    }
}
