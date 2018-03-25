//! Module that adds fasterxml annotations to generated classes.

use codegen::{Configure, ServiceAdded, ServiceCodegen};
use core::Loc;
use core::errors::*;
use flavored::JavaEndpoint;
use genco::{Cons, IntoTokens, Java, Quoted, Tokens};
use genco::java::{imported, local, Argument, Class, Constructor, Field, Method, Modifier, VOID};
use naming::{self, Naming};
use processor::Processor;
use std::borrow::Borrow;
use std::rc::Rc;
use utils::Override;

const CLIENT_STUB_NAME: &'static str = "ClientStub";
const SERVER_STUB_NAME: &'static str = "ServerStub";

pub struct Module;

impl Module {
    pub fn initialize(self, e: Configure) {
        e.options.suppress_service_methods = true;
        e.options
            .service_generators
            .push(Box::new(GrpcClient::new()));
    }
}

pub enum MethodType {
    Unary,
    ClientStreaming,
    ServerStreaming,
    BidiStreaming,
    Unknown,
}

impl MethodType {
    pub fn variant_name(&self) -> &'static str {
        use self::MethodType::*;

        match *self {
            Unary => "UNARY",
            ClientStreaming => "CLIENT_STREAMING",
            ServerStreaming => "SERVER_STREAMING",
            BidiStreaming => "BIDI_STREAMING",
            Unknown => "UNKNOWN",
        }
    }
}

/// @Generated("message") annotation
struct Generated<'a, 'el>(&'a GrpcClient, Cons<'el>);

impl<'a, 'el> IntoTokens<'el, Java<'el>> for Generated<'a, 'el> {
    fn into_tokens(self) -> Tokens<'el, Java<'el>> {
        toks!["@", self.0.generated.clone(), "(", self.1.quoted(), ")"]
    }
}

/// Embedded marshaller for Void.
struct VoidMarshaller<'a>(&'a GrpcClient);

impl<'a, 'el> IntoTokens<'el, Java<'el>> for VoidMarshaller<'a> {
    fn into_tokens(self) -> Tokens<'el, Java<'el>> {
        use self::Modifier::*;

        let mut class = Class::new("VoidMarshaller");
        class.implements = vec![self.0.marshaller.with_arguments(vec![VOID.as_boxed()])];
        class.modifiers.push(Static);

        // parse
        class.methods.push({
            let mut m = Method::new("parse");
            m.annotation(Override);
            m.arguments
                .push(Argument::new(self.0.input_stream.clone(), "stream"));
            m.returns = VOID.as_boxed();

            m.body.push("return null;");
            m
        });

        // stream
        class.methods.push({
            let mut m = Method::new("stream");
            m.annotation(Override);
            m.returns = self.0.input_stream.clone();
            m.arguments.push(Argument::new(VOID.as_boxed(), "value"));

            m.body
                .push(toks!["return new ", self.0.bais.clone(), "(new byte[0]);",]);
            m
        });

        class.into_tokens()
    }
}

/// Embedded marshaller for Json.
struct JsonMarshaller<'a>(&'a GrpcClient);

impl<'a, 'el> IntoTokens<'el, Java<'el>> for JsonMarshaller<'a> {
    fn into_tokens(self) -> Tokens<'el, Java<'el>> {
        use self::Modifier::*;

        let tpl = local("T");

        let mapper = Field::new(self.0.object_mapper.clone(), "mapper");
        let ty = Field::new(
            self.0.type_reference.with_arguments(vec![tpl.clone()]),
            "type",
        );

        let mut class = Class::new("JsonMarshaller");
        class.parameters.append(tpl.clone());
        class.implements = vec![self.0.marshaller.with_arguments(vec![tpl.clone()])];
        class.modifiers.push(Static);

        class.fields.push(mapper);
        class.fields.push(ty.clone());

        class.constructors.push({
            let mut c = Constructor::new();

            c.arguments.push(Argument::new(ty.ty(), ty.var()));

            c.body.push(toks![
                "this.mapper = ",
                self.0.mapper_provider.clone(),
                ".get();",
            ]);
            c.body.push(toks!["this.", ty.var(), " = ", ty.var(), ";"]);
            c
        });

        // parse
        class.methods.push({
            let mut m = Method::new("parse");
            m.annotation(Override);
            m.returns = tpl.clone();
            m.arguments
                .push(Argument::new(self.0.input_stream.clone(), "stream"));

            m.body.push({
                let mut t = Tokens::new();
                t.push("try {");
                t.nested(toks![
                    "return this.mapper.readValue(stream, this.",
                    ty.var(),
                    ");",
                ]);
                t.push("} catch (final Exception e) {");
                t.nested("throw new RuntimeException(e);");
                t.push("}");
                t
            });

            m
        });

        // stream
        class.methods.push({
            let mut m = Method::new("stream");
            m.annotation(Override);
            m.returns = self.0.input_stream.clone();
            m.arguments.push(Argument::new(tpl.clone(), "value"));

            m.body.push({
                let mut t = Tokens::new();
                t.push("final byte[] bytes;");
                t.push("try {");
                t.nested("bytes = this.mapper.writeValueAsBytes(value);");
                t.push("} catch (final Exception e) {");
                t.nested("throw new RuntimeException(e);");
                t.push("}");
                t.push(toks!["return new ", self.0.bais.clone(), "(bytes);"]);
                t
            });

            m
        });

        class.into_tokens()
    }
}

pub struct GrpcClient {
    to_upper_snake: naming::ToUpperSnake,
    mapper_provider: Java<'static>,
    bais: Java<'static>,
    generated: Java<'static>,
    abstract_stub: Java<'static>,
    channel: Java<'static>,
    call_options: Java<'static>,
    bindable_service: Java<'static>,
    server_service_definition: Java<'static>,
    client_calls: Java<'static>,
    server_calls: Java<'static>,
    stream_observer: Java<'static>,
    method_descriptor: Java<'static>,
    marshaller: Java<'static>,
    input_stream: Java<'static>,
    object_mapper: Java<'static>,
    type_reference: Java<'static>,
}

impl GrpcClient {
    pub fn new() -> GrpcClient {
        GrpcClient {
            to_upper_snake: naming::to_upper_snake(),
            mapper_provider: imported("io.reproto", "MapperProvider"),
            bais: imported("java.io", "ByteArrayInputStream"),
            generated: imported("javax.annotation", "Generated"),
            abstract_stub: imported("io.grpc.stub", "AbstractStub"),
            channel: imported("io.grpc", "Channel"),
            call_options: imported("io.grpc", "CallOptions"),
            bindable_service: imported("io.grpc", "BindableService"),
            server_service_definition: imported("io.grpc", "ServerServiceDefinition"),
            client_calls: imported("io.grpc.stub", "ClientCalls"),
            server_calls: imported("io.grpc.stub", "ServerCalls"),
            stream_observer: imported("io.grpc.stub", "StreamObserver"),
            method_descriptor: imported("io.grpc", "MethodDescriptor"),
            marshaller: imported("io.grpc", "MethodDescriptor.Marshaller"),
            input_stream: imported("java.io", "InputStream"),
            object_mapper: imported("com.fasterxml.jackson.databind", "ObjectMapper"),
            type_reference: imported("com.fasterxml.jackson.core.type", "TypeReference"),
        }
    }

    /// Get the MethodType variant for the given endpoint.
    fn method_type(&self, e: &Loc<JavaEndpoint>) -> Result<MethodType> {
        use core::RpChannel::*;

        let request = e.request.as_ref().map(|v| Loc::value(&v.channel));
        let response = e.response.as_ref().map(|v| Loc::value(v));

        let out = match (request, response) {
            (Some(&Unary { .. }), Some(&Unary { .. })) => MethodType::Unary,
            (Some(&Streaming { .. }), Some(&Unary { .. })) => MethodType::ClientStreaming,
            (Some(&Unary { .. }), Some(&Streaming { .. })) => MethodType::ServerStreaming,
            (Some(&Streaming { .. }), Some(&Streaming { .. })) => MethodType::BidiStreaming,
            _ => MethodType::Unknown,
        };

        Ok(out)
    }

    /// Build the method descriptor field.
    fn method_field<'el>(
        &self,
        service_name: Rc<String>,
        method_type: &MethodType,
        e: &'el Loc<JavaEndpoint>,
    ) -> Field<'el> {
        use self::Modifier::*;

        let method_name = Rc::new(format!("METHOD_{}", self.to_upper_snake.convert(&e.ident)));

        let descriptor_ty = self.method_descriptor
            .with_arguments(vec![e.request_ty.clone(), e.response_ty.clone()]);

        let mut field = Field::new(descriptor_ty, method_name.clone());
        field.modifiers = vec![Public, Static, Final];

        field.initializer({
            let mut init = Tokens::new();

            push!(
                init,
                self.method_descriptor,
                ".<",
                e.request_ty.as_boxed(),
                ", ",
                e.response_ty.as_boxed(),
                ">newBuilder()"
            );

            init.nested({
                let mut t = Tokens::new();
                t.push(toks![
                    ".setType(",
                    self.method_descriptor.clone(),
                    ".MethodType.",
                    method_type.variant_name(),
                    ")",
                ]);

                t.push(toks![
                    ".setFullMethodName(",
                    self.method_descriptor.clone(),
                    ".generateFullMethodName(",
                    service_name.quoted(),
                    ", ",
                    e.name.clone().quoted(),
                    "))",
                ]);

                if e.request_ty != VOID {
                    t.push(toks![
                        ".setRequestMarshaller(new JsonMarshaller(",
                        "new ",
                        self.type_reference
                            .with_arguments(vec![e.request_ty.clone()]),
                        "(){}",
                        "))",
                    ]);
                } else {
                    t.push(".setRequestMarshaller(new VoidMarshaller())");
                }

                if e.response_ty != VOID {
                    t.push(toks![
                        ".setResponseMarshaller(new JsonMarshaller(",
                        "new ",
                        self.type_reference
                            .with_arguments(vec![e.response_ty.clone()]),
                        "(){}",
                        "))",
                    ]);
                } else {
                    t.push(".setResponseMarshaller(new VoidMarshaller())");
                }

                t.push(".build();");
                t
            });

            let mut t = Tokens::new();
            t.nested(init);
            t
        });

        field
    }

    fn is_not_whitespace(c: char) -> bool {
        !c.is_whitespace()
    }

    fn javadoc_comments<'el, S: Borrow<str>>(out: &mut Vec<Cons<'el>>, input: &'el [S]) {
        if input.is_empty() {
            return;
        }

        let offset = input
            .iter()
            .flat_map(|line| line.borrow().find(Self::is_not_whitespace).into_iter())
            .min();
        let offset = offset.unwrap_or(0usize);

        out.push("<pre>".into());

        for comment in input {
            let comment = comment.borrow();
            out.push(comment[usize::min(offset, comment.len())..].into());
        }

        out.push("</pre>".into());
    }

    /// Build the client method helper for making calls.
    ///
    /// This is built differently depending on which MethodType has been used.
    fn client_method<'el>(
        &self,
        field: &Field<'el>,
        method_type: &MethodType,
        e: &'el Loc<JavaEndpoint>,
    ) -> Method<'el> {
        use self::MethodType::*;
        use self::Modifier::*;

        let mut method = Method::new(e.ident.clone());
        method.modifiers = vec![Public];

        Self::javadoc_comments(&mut method.comments, &e.comment);

        let request_observer_ty = self.stream_observer
            .with_arguments(vec![e.request_ty.clone()]);

        let observer_ty = self.stream_observer
            .with_arguments(vec![e.response_ty.clone()]);

        let request_arg = Argument::new(e.request_ty.clone(), "request");
        let observer_arg = Argument::new(observer_ty, "observer");

        let new_call = toks!["getChannel().newCall(", field.var(), ", getCallOptions())"];

        let mut args = Tokens::new();
        args.append(new_call);

        match *method_type {
            Unary => {
                args.append(request_arg.var());
                args.append(observer_arg.var());

                method.arguments.push(request_arg);
                method.arguments.push(observer_arg);

                method.body.push(toks![
                    self.client_calls.clone(),
                    ".asyncUnaryCall(",
                    args.join(", "),
                    ");",
                ]);
            }
            ClientStreaming => {
                args.append(observer_arg.var());

                method.arguments.push(observer_arg);
                method.returns = request_observer_ty;

                method.body.push(toks![
                    "return ",
                    self.client_calls.clone(),
                    ".asyncClientStreamingCall(",
                    args.join(", "),
                    ");",
                ]);
            }
            ServerStreaming => {
                args.append(request_arg.var());
                args.append(observer_arg.var());

                method.arguments.push(request_arg);
                method.arguments.push(observer_arg);

                method.body.push(toks![
                    self.client_calls.clone(),
                    ".asyncServerStreamingCall(",
                    args.join(", "),
                    ");",
                ]);
            }
            Unknown | BidiStreaming => {
                args.append(observer_arg.var());

                method.arguments.push(observer_arg);
                method.returns = request_observer_ty;

                method.body.push(toks![
                    "return ",
                    self.client_calls.clone(),
                    ".asyncBidiStreamingCall(",
                    args.join(", "),
                    ");",
                ]);
            }
        }

        method
    }

    /// Build the server method that will handle the request.
    fn server_method<'el>(
        &self,
        field: &Field<'el>,
        method_type: &MethodType,
        e: &'el Loc<JavaEndpoint>,
    ) -> Method<'el> {
        use self::MethodType::*;
        use self::Modifier::*;

        let mut method = Method::new(e.ident.clone());
        method.modifiers = vec![Public];

        Self::javadoc_comments(&mut method.comments, &e.comment);

        let request_observer_ty = self.stream_observer
            .with_arguments(vec![e.request_ty.clone()]);

        let observer_ty = self.stream_observer
            .with_arguments(vec![e.response_ty.clone()]);

        let request_arg = Argument::new(e.request_ty.clone(), "request");
        let observer_arg = Argument::new(observer_ty, "observer");

        let mut args = toks![field.var()];

        match *method_type {
            // Takes an argument which is the immediate request.
            Unary | ServerStreaming => {
                args.append(observer_arg.var());

                method.arguments.push(request_arg);
                method.arguments.push(observer_arg);

                method.body.push(toks![
                    self.server_calls.clone(),
                    ".asyncUnimplementedUnaryCall(",
                    args.join(", "),
                    ");",
                ]);
            }
            // All streaming identical streaming implementations.
            ClientStreaming | BidiStreaming | Unknown => {
                args.append(observer_arg.var());

                method.arguments.push(observer_arg);
                method.returns = request_observer_ty;

                method.body.push(toks![
                    "return ",
                    self.server_calls.clone(),
                    ".asyncUnimplementedStreamingCall(",
                    args.join(", "),
                    ");",
                ]);
            }
        }

        method
    }

    /// Setup the class corresponding to the client stub.
    fn client_stub<'el>(&self) -> Class<'el> {
        use self::Modifier::*;

        let name = CLIENT_STUB_NAME;

        let mut spec = Class::new(name.clone());
        spec.annotation(Generated(self, "Generated by reproto".into()));
        spec.modifiers = vec![Static];
        spec.extends = Some(self.abstract_stub.with_arguments(vec![local(name.clone())]));

        spec.constructors.push({
            let mut c = Constructor::new();

            c.arguments
                .push(Argument::new(self.channel.clone(), "channel"));

            c.body.push("super(channel);");

            c
        });

        spec.constructors.push({
            let mut c = Constructor::new();

            c.arguments
                .push(Argument::new(self.channel.clone(), "channel"));

            c.arguments
                .push(Argument::new(self.call_options.clone(), "callOptions"));

            c.body.push("super(channel, callOptions);");

            c
        });

        spec.methods.push({
            let mut m = Method::new("build");
            m.modifiers = vec![Protected];
            m.annotation(Override);
            m.returns = local(name.clone());

            m.arguments
                .push(Argument::new(self.channel.clone(), "channel"));

            m.arguments
                .push(Argument::new(self.call_options.clone(), "callOptions"));

            m.body.push(toks![
                "return new ",
                name.clone(),
                "(channel, callOptions);",
            ]);

            m
        });

        spec
    }

    /// Setup the class corresponding to the server stub.
    fn server_stub<'el>(&self) -> Class<'el> {
        use self::Modifier::*;

        let name = SERVER_STUB_NAME;

        let mut spec = Class::new(name);
        spec.annotation(Generated(self, "Generated by reproto".into()));
        spec.modifiers = vec![Static, Abstract];
        spec.implements = vec![self.bindable_service.clone()];

        spec
    }

    /// Build the addMethod call for a given endpoint to populate the server definition.
    fn server_definition_add_method<'el>(
        &self,
        ident: Cons<'el>,
        field: &Field<'el>,
        method_type: &MethodType,
    ) -> Tokens<'el, Java<'el>> {
        use self::MethodType::*;

        let mut args = toks![field.var()];

        match *method_type {
            Unary => {
                args.append(toks![
                    self.server_calls.clone(),
                    ".asyncUnaryCall(this::",
                    ident,
                    ")",
                ]);
            }
            ClientStreaming => {
                args.append(toks![
                    self.server_calls.clone(),
                    ".asyncClientStreamingCall(this::",
                    ident,
                    ")",
                ]);
            }
            ServerStreaming => {
                args.append(toks![
                    self.server_calls.clone(),
                    ".asyncServerStreamingCall(this::",
                    ident,
                    ")",
                ]);
            }
            Unknown | BidiStreaming => {
                args.append(toks![
                    self.server_calls.clone(),
                    ".asyncBidiStreamingCall(this::",
                    ident,
                    ")",
                ]);
            }
        }

        toks![".addMethod(", args.join(", "), ")"]
    }
}

impl Processor for GrpcClient {}

impl ServiceCodegen for GrpcClient {
    fn generate(&self, e: ServiceAdded) -> Result<()> {
        let ServiceAdded { body, spec, .. } = e;

        let mut client_stub = self.client_stub();
        let mut server_stub = self.server_stub();

        let mut bind_service = Method::new("bindService");
        bind_service.returns = self.server_service_definition.clone();
        bind_service.annotation(Override);

        bind_service
            .body
            .push(toks!["return ", self.server_service_definition.clone(),]);

        let service_name = Rc::new(format!(
            "{}.{}",
            self.java_package(&body.name.package).parts.join("."),
            body.name.join(".")
        ));

        bind_service
            .body
            .nested(toks![".builder(", service_name.clone().quoted(), ")",]);

        for e in &body.endpoints {
            let method_type = self.method_type(e)?;

            let field = self.method_field(service_name.clone(), &method_type, e);

            let server_method = self.server_method(&field, &method_type, e);
            let client_method = self.client_method(&field, &method_type, e);

            bind_service.body.nested(self.server_definition_add_method(
                e.ident.clone(),
                &field,
                &method_type,
            ));

            spec.body.push(field);
            server_stub.methods.push(server_method);
            client_stub.methods.push(client_method);
        }

        bind_service.body.nested(".build();");

        server_stub.body.push(bind_service);

        spec.body.push(client_stub);
        spec.body.push(server_stub);
        spec.body.push(JsonMarshaller(self));
        spec.body.push(VoidMarshaller(self));
        Ok(())
    }
}
