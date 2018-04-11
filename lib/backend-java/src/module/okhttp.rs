//! Module that adds fasterxml annotations to generated classes.

use codegen::{Codegen, Configure, ServiceAdded, ServiceCodegen};
use core::errors::*;
use core::{self, Handle, Loc};
use flavored::{JavaEndpoint, RpEndpointHttp1, RpPackage, RpPathStep};
use genco::java::{imported, local, Argument, Class, Constructor, Field, Interface, Method,
                  Modifier, VOID};
use genco::{Cons, IntoTokens, Java, Quoted, Tokens};
use java_file::JavaFile;
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

        e.options
            .root_generators
            .push(Box::new(OkHttpSerialization::new()));
    }
}

/// Model for a nested `Builder` class.
pub struct Builder<'el> {
    ty: Java<'el>,
    optional: Java<'el>,
    client: Field<'el>,
    base_url: Field<'el>,
    ser: Field<'el>,
}

impl<'el> Builder<'el> {
    fn build(&self) -> Method<'el> {
        macro_rules! opt_check {
            ($d:expr, $v:expr) => {
                push!(
                    $d,
                    "final ",
                    $v.ty().as_value(),
                    " ",
                    $v.var(),
                    " = ",
                    required(&$v),
                    ";"
                )
            };
        }

        let mut args = Tokens::new();

        args.append(self.client.var());
        args.append(self.base_url.var());
        args.append(self.ser.var());

        let mut build = Method::new("build");

        build.returns = self.ty.clone();

        opt_check!(build.body, self.base_url);
        opt_check!(build.body, self.ser);

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
        let ser = self.field(self.ser.var(), self.ser.ty());

        builder.fields.push(base_url.clone());
        builder.fields.push(ser.clone());

        builder
            .methods
            .push(self.setter(self.base_url.var(), self.base_url.ty()));
        builder
            .methods
            .push(self.setter(self.ser.var(), self.ser.ty()));

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

pub struct OkHttpSerialization {
    request_body: Java<'static>,
    response_body: Java<'static>,
    cls: Java<'static>,
}

impl OkHttpSerialization {
    pub fn new() -> Self {
        Self {
            request_body: imported("okhttp3", "RequestBody"),
            response_body: imported("okhttp3", "ResponseBody"),
            cls: imported("java.lang", "Class"),
        }
    }
}

impl Codegen for OkHttpSerialization {
    fn generate(&self, handle: &Handle) -> Result<()> {
        let package = RpPackage::parse("io.reproto");

        JavaFile::new(package, "OkHttpSerialization", |out| {
            let mut c = Interface::new("OkHttpSerialization");

            let t = local("T");

            let entity = Argument::new(t.clone(), "entity");

            let encode = {
                let mut m = Method::new("encode");
                m.comments.push("Encode the request body.".into());
                m.parameters.append(t.clone());
                m.arguments.push(entity.clone());
                m.returns = self.request_body.clone();
                m.modifiers = vec![];
                m
            };

            let body = Argument::new(self.response_body.clone(), "body");
            let cls = Argument::new(self.cls.with_arguments(vec![t.clone()]), "cls");

            let decode = {
                let mut m = Method::new("decode");
                m.comments.push("Decode the response body.".into());
                m.parameters.append(t.clone());
                m.arguments.push(body.clone());
                m.arguments.push(cls.clone());
                m.returns = t.clone();
                m.modifiers = vec![];
                m
            };

            c.methods.push(encode.clone());
            c.methods.push(decode.clone());

            c.body.push(Jackson::new(encode, decode, body, cls, entity));

            out.push(c);
            Ok(())
        }).process(handle)?;

        return Ok(());

        pub struct Jackson<'el> {
            encode: Method<'el>,
            decode: Method<'el>,
            body: Argument<'el>,
            cls: Argument<'el>,
            entity: Argument<'el>,
            request_body: Java<'static>,
            object_mapper: Java<'static>,
            runtime_exc: Java<'static>,
            io_exc: Java<'static>,
            media_type: Java<'static>,
        }

        impl<'el> Jackson<'el> {
            pub fn new(
                encode: Method<'el>,
                decode: Method<'el>,
                body: Argument<'el>,
                cls: Argument<'el>,
                entity: Argument<'el>,
            ) -> Jackson<'el> {
                Jackson {
                    encode,
                    decode,
                    body,
                    cls,
                    entity,
                    request_body: imported("okhttp3", "RequestBody"),
                    object_mapper: imported("com.fasterxml.jackson.databind", "ObjectMapper"),
                    runtime_exc: imported("java.lang", "RuntimeException"),
                    io_exc: imported("java.io", "IOException"),
                    media_type: imported("okhttp3", "MediaType"),
                }
            }
        }

        impl<'el> IntoTokens<'el, Java<'el>> for Jackson<'el> {
            fn into_tokens(self) -> Tokens<'el, Java<'el>> {
                let mut c = Class::new("Jackson");

                let mut json = Field::new(self.media_type.clone(), "JSON");
                json.modifiers = vec![Modifier::Private, Modifier::Static, Modifier::Final];
                json.initializer(toks![
                    self.media_type.clone(),
                    ".parse(",
                    "application/json; charset=utf-8".quoted(),
                    ")"
                ]);

                let f = Field::new(self.object_mapper.clone(), "m");
                let m = Argument::new(self.object_mapper.clone(), "m");

                c.implements = vec![local("OkHttpSerialization")];
                c.fields.push(json.clone());
                c.fields.push(f.clone());

                c.constructors.push({
                    let mut c = Constructor::new();

                    c.arguments.push(m.clone());
                    push!(c.body, "this.", f.var(), " = ", m.var(), ";");

                    c
                });

                c.methods.push({
                    let mut m = self.decode.clone();
                    m.annotation(Override);
                    m.modifiers = vec![Modifier::Public];

                    let read = toks![
                        "m.readValue(",
                        self.body.var(),
                        ".bytes(), ",
                        self.cls.var(),
                        ")"
                    ];

                    push!(m.body, "try {");
                    nested!(m.body, "return ", read, ";");
                    push!(m.body, "} catch (final ", self.io_exc, " e) {");
                    nested!(m.body, "throw new ", self.runtime_exc, "(e);");
                    push!(m.body, "}");

                    m
                });

                c.methods.push({
                    let mut m = self.encode.clone();
                    m.annotation(Override);
                    m.modifiers = vec![Modifier::Public];

                    let write = toks!["m.writeValueAsBytes(", self.entity.var(), ")"];

                    push!(m.body, "final byte[] buffer;");

                    m.body.push({
                        let mut t = Tokens::new();

                        push!(t, "try {");
                        nested!(t, "buffer = ", write, ";");
                        push!(t, "} catch (final ", self.io_exc, " e) {");
                        nested!(t, "throw new ", self.runtime_exc, "(e);");
                        push!(t, "}");

                        t
                    });

                    push!(
                        m.body,
                        "return ",
                        self.request_body,
                        ".create(",
                        json.var(),
                        ", buffer);"
                    );

                    m.body = m.body.join_line_spacing();

                    m
                });

                c.into_tokens()
            }
        }
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
    optional: Java<'static>,
    completable_future: Java<'static>,
    serialization: Java<'static>,
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
            optional: imported("java.util", "Optional"),
            completable_future: imported("java.util.concurrent", "CompletableFuture"),
            serialization: imported("io.reproto", "OkHttpSerialization"),
        }
    }
}

impl OkHttpServiceCodegen {
    fn request<'el>(
        &self,
        mut method: Method<'el>,
        e: &'el Loc<JavaEndpoint>,
        http: &'el RpEndpointHttp1,
        client: Field<'el>,
        base_url: Field<'el>,
    ) -> Result<Method<'el>> {
        let ser = "OkHttp.this.serialization";

        let request_var = e.request
            .as_ref()
            .map(|r| toks![ser.clone(), ".encode(", r.safe_ident(), ")"])
            .unwrap_or_else(|| toks!["null"]);

        let response_var = e.response
            .as_ref()
            .map(|r| toks![ser.clone(), ".decode(response.body(), ", r.ty(), ".class)"])
            .unwrap_or_else(|| toks!["null"]);

        method.body.push({
            let mut t = Tokens::new();

            t.push(toks![
                "final ",
                self.http_url.clone(),
                " url_ = this.",
                base_url.var(),
                ".newBuilder()",
            ]);

            for step in &http.path.steps {
                let args = step_args(step)?;
                t.nested(toks![".addPathSegment(", args.join(" + "), ")"]);
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

            let method = http.method.as_str();

            nested!(t, ".url(url_)");
            // TODO: actually provide the body
            nested!(t, ".method(", method.quoted(), ", ", request_var, ")");
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
                            " + response)",
                        ];

                        t.push("if (!response.isSuccessful()) {");
                        nested!(t, "future_.completeExceptionally(", exc, ");");
                        t.push("} else {");
                        // TODO: complete with the response
                        nested!(t, "future_.complete(", response_var, ");");
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

        fn step_args<'el>(step: &'el RpPathStep) -> Result<Tokens<'el, Java<'el>>> {
            let mut args = Tokens::new();

            for part in &step.parts {
                match *part {
                    core::RpPathPart::Variable(ref arg) => {
                        let ty = arg.channel.ty();

                        if ty.is_primitive() {
                            args.append(toks![ty.as_boxed(), ".toString(", arg.safe_ident(), ")"]);
                        } else {
                            args.append(arg.safe_ident());
                        }
                    }
                    core::RpPathPart::Segment(ref s) => {
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
        let ServiceAdded { body, spec, .. } = e;

        let client = Field::new(self.client.clone(), "client");
        let base_url = Field::new(self.http_url.clone(), "baseUrl");
        let ser = Field::new(self.serialization.clone(), "serialization");

        let ok_http = {
            let mut c = Class::new("OkHttp");
            c.implements = vec![local(spec.name().to_string())];

            for e in &body.endpoints {
                if let Some(http) = e.http1.as_ref() {
                    let mut m = Method::new(e.safe_ident());
                    m.returns = self.completable_future
                        .with_arguments(vec![http.response.as_ref().unwrap_or(&VOID).clone()]);
                    m.arguments.extend(e.arguments.iter().cloned());

                    c.methods
                        .push(self.request(m, e, http, client.clone(), base_url.clone())?);
                }
            }

            c.constructors.push({
                let mut c = Constructor::new();
                let client_arg = Argument::new(client.ty(), client.var());
                let base_arg = Argument::new(base_url.ty(), base_url.var());
                let ser_arg = Argument::new(ser.ty(), ser.var());

                push!(c.body, "this.", client.var(), " = ", client_arg.var(), ";");
                push!(c.body, "this.", base_url.var(), " = ", base_arg.var(), ";");
                push!(c.body, "this.", ser.var(), " = ", ser_arg.var(), ";");

                c.arguments.push(client_arg);
                c.arguments.push(base_arg);
                c.arguments.push(ser_arg);
                c
            });

            c.fields.push(client.clone());
            c.fields.push(base_url.clone());
            c.fields.push(ser.clone());
            c
        };

        spec.body.push(ok_http);

        spec.body.push(Builder {
            ty: local("OkHttp"),
            optional: self.optional.clone(),
            client: client,
            base_url: base_url,
            ser: ser,
        });

        Ok(())
    }
}
