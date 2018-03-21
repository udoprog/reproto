//! Java backend for reproto

use Options;
use backend::Converter;
use codegen::{ClassAdded, EndpointExtra, EnumAdded, GetterAdded, InterfaceAdded, ServiceAdded,
              TupleAdded};
use core::{ForEachLoc, Handle, Loc, RpCode, RpContext, RpDecl, RpEnumBody, RpEnumType, RpField,
           RpInterfaceBody, RpName, RpServiceBody, RpTupleBody, RpTypeBody, WithPos};
use core::errors::*;
use genco::{Cons, Element, Java, Quoted, Tokens};
use genco::java::{imported, local, optional, Argument, Class, Constructor, Enum, Field, Interface,
                  Method, Modifier, BOOLEAN, INTEGER};
use java_field::JavaField;
use java_file::JavaFile;
use naming::{self, Naming};
use processor::Processor;
use std::rc::Rc;
use trans::Environment;
use utils::{Observer, Override, Utils};

/// Helper macro to implement listeners opt loop.
fn code<'el>(codes: &'el [Loc<RpCode>]) -> Tokens<'el, Java<'el>> {
    let mut t = Tokens::new();

    for c in codes {
        if let RpContext::Java { ref imports, .. } = c.context {
            for import in imports {
                if let Some(split) = import.rfind('.') {
                    let (package, name) = import.split_at(split);
                    let name = &name[1..];
                    t.register(imported(package, name));
                }
            }

            // TODO: explicitly include imports through genco. Tokens::opaque?

            t.append({
                let mut t = Tokens::new();

                for line in &c.lines {
                    t.push(line.as_str());
                }

                t
            });
        }
    }

    t
}

macro_rules! call_codegen {
    ($source: expr, $event: expr) => {
        for g in $source {
            g.generate($event)?;
        }
    };
}

pub struct Compiler<'el> {
    env: &'el Environment,
    variant_field: &'el Loc<RpField>,
    utils: &'el Utils,
    options: Options,
    to_upper_camel: naming::ToUpperCamel,
    to_lower_camel: naming::ToLowerCamel,
    variant_naming: naming::ToUpperSnake,
    null_string: Element<'static, Java<'static>>,
    suppress_warnings: Java<'static>,
    string_builder: Java<'static>,
    pub void: Java<'static>,
    objects: Java<'static>,
    object: Java<'static>,
    string: Java<'static>,
    pub optional: Java<'static>,
    illegal_argument: Java<'static>,
    observer: Java<'static>,
}

impl<'el> Processor for Compiler<'el> {}

impl<'el> Compiler<'el> {
    pub fn new(
        env: &'el Environment,
        variant_field: &'el Loc<RpField>,
        utils: &'el Utils,
        options: Options,
    ) -> Compiler<'el> {
        Compiler {
            env: env,
            variant_field: variant_field,
            utils: utils,
            options: options,
            to_upper_camel: naming::to_upper_camel(),
            to_lower_camel: naming::to_lower_camel(),
            variant_naming: naming::to_upper_snake(),
            null_string: "null".quoted(),
            void: imported("java.lang", "Void"),
            objects: imported("java.util", "Objects"),
            suppress_warnings: imported("java.lang", "SuppressWarnings"),
            string_builder: imported("java.lang", "StringBuilder"),
            object: imported("java.lang", "Object"),
            string: imported("java.lang", "String"),
            optional: imported("java.util", "Optional"),
            illegal_argument: imported("java.lang", "IllegalArgumentException"),
            observer: imported("reproto.io", "Observer"),
        }
    }

    pub fn compile(&self, handle: &Handle) -> Result<()> {
        for generator in &self.options.root_generators {
            generator.generate(handle)?;
        }

        JavaFile::new("io.reproto", "Observer", |out| {
            out.push(Observer);
            Ok(())
        }).process(handle)?;

        for decl in self.env.toplevel_decl_iter() {
            self.compile_decl(handle, decl).with_pos(decl.pos())?;
        }

        Ok(())
    }

    fn compile_decl(&self, handle: &Handle, decl: &RpDecl) -> Result<()> {
        let package_name = self.java_package(&decl.name().package).parts.join(".");

        JavaFile::new(package_name.as_str(), decl.ident(), |out| {
            self.process_decl(decl, 0usize, out)
        }).process(handle)
    }

    fn field_mods(&self) -> Vec<Modifier> {
        use self::Modifier::*;

        if self.options.immutable {
            vec![Private, Final]
        } else {
            vec![Private]
        }
    }

    fn new_field_spec(&self, ty: &Java<'el>, name: &'el str) -> Field<'el> {
        let mut field = Field::new(ty.clone(), name);
        field.modifiers = self.field_mods();
        field
    }

    fn build_constructor(&self, fields: &[JavaField<'el>]) -> Constructor<'el> {
        let mut c = Constructor::new();

        for field in fields {
            let spec = &field.spec;

            let argument = Argument::new(spec.ty(), spec.var());

            if !self.options.nullable {
                if let Some(non_null) = self.require_non_null(spec, &argument, field.name().into())
                {
                    c.body.push(non_null);
                }
            }

            c.arguments.push(argument.clone());

            c.body
                .push(toks!["this.", field.spec.var(), " = ", argument.var(), ";",]);
        }

        c
    }

    /// Build a require-non-null check.
    fn require_non_null(
        &self,
        field: &Field<'el>,
        argument: &Argument<'el>,
        name: Cons<'el>,
    ) -> Option<Tokens<'el, Java<'el>>> {
        use self::Java::*;

        match field.ty() {
            Primitive { .. } => None,
            _ => {
                let req = toks![self.objects.clone(), ".requireNonNull"];

                Some(toks![req, "(", argument.var(), ", ", name.quoted(), ");",])
            }
        }
    }

    fn build_hash_code(&self, fields: &[JavaField<'el>]) -> Method<'el> {
        let mut hash_code = Method::new("hashCode");

        hash_code.annotation(Override);
        hash_code.returns = INTEGER;

        hash_code.body.push("int result = 1;");

        for field in fields {
            let field = &field.spec;

            let field_toks = toks!["this.", field.var()];

            let value = match field.ty() {
                ref primitive @ Java::Primitive { .. } => {
                    if *primitive == INTEGER {
                        field_toks.clone()
                    } else {
                        toks![primitive.as_boxed(), ".hashCode(", field_toks.clone(), ")"]
                    }
                }
                _ => toks![field_toks.clone(), ".hashCode()"],
            };

            let value = if self.options.nullable {
                match field.ty() {
                    Java::Primitive { .. } => value,
                    _ => toks!["(", field_toks.clone(), " != null ? 0 : ", value, ")"],
                }
            } else {
                value
            };

            hash_code
                .body
                .push(toks!["result = result * 31 + ", value, ";"]);
        }

        hash_code.body.push("return result;");
        hash_code
    }

    fn build_equals(&self, name: Cons<'el>, fields: &[JavaField<'el>]) -> Method<'el> {
        let argument = Argument::new(self.object.clone(), "other");

        let mut equals = Method::new("equals");

        equals.annotation(Override);
        equals.returns = BOOLEAN;
        equals.arguments.push(argument.clone());

        // check if argument is null.
        {
            let mut null_check = Tokens::new();

            null_check.push(toks!["if (", argument.var(), " == null) {"]);
            null_check.nested("return false;");
            null_check.push("}");

            equals.body.push(null_check);
        }

        // check that argument is expected type.
        equals.body.push({
            let mut t = Tokens::new();

            t.push(toks![
                "if (!(",
                argument.var(),
                " instanceof ",
                name.clone(),
                ")) {",
            ]);
            t.nested("return false;");
            t.push("}");
            t
        });

        // cast argument.
        equals.body.push({
            let mut t = Tokens::new();

            t.push(toks![
                "@",
                self.suppress_warnings.clone(),
                "(",
                "unchecked".quoted(),
                ")",
            ]);

            t.push(toks![
                "final ",
                name.clone(),
                " o = (",
                name.clone(),
                ") ",
                argument.var(),
                ";",
            ]);

            t
        });

        for field in fields {
            let field = &field.spec;
            let field_toks = toks!["this.", field.var()];
            let o = toks!["o.", field.var()];

            let equals_condition = match field.ty() {
                Java::Primitive { .. } => toks![field_toks.clone(), " != ", o.clone()],
                _ => toks!["!", field_toks.clone(), ".equals(", o.clone(), ")"],
            };

            let mut equals_check = Tokens::new();

            equals_check.push(toks!["if (", equals_condition, ") {"]);
            equals_check.nested("return false;");
            equals_check.push("}");

            if self.options.nullable {
                let mut null_check = Tokens::new();

                null_check.push(toks!["if (", o, " != null) {"]);
                null_check.nested("return false;");
                null_check.push("}");

                let mut field_check = Tokens::new();

                field_check.push(toks!["if (", field_toks, " == null) {"]);
                field_check.nested(null_check);
                field_check.push("} else {");
                field_check.nested(equals_check);
                field_check.push("}");

                equals.body.push(field_check);
            } else {
                equals.body.push(equals_check);
            }
        }

        equals.body.push("return true;");
        equals.body = equals.body.join_line_spacing();

        equals
    }

    fn build_to_string(&self, name: Cons<'el>, fields: &[JavaField<'el>]) -> Method<'el> {
        let mut to_string = Method::new("toString");

        to_string.annotation(Override);
        to_string.returns = self.string.clone();

        to_string.body.push(toks![
            "final ",
            self.string_builder.clone(),
            " b = new ",
            self.string_builder.clone(),
            "();",
        ]);

        let mut body = Tokens::new();

        for field in fields {
            let field_toks = toks!["this.", field.spec.var()];

            let format = match field.spec.ty() {
                java @ Java::Primitive { .. } => {
                    toks![java.as_boxed(), ".toString(", field_toks.clone(), ")"]
                }
                _ => {
                    let format = toks![field_toks.clone(), ".toString()"];

                    if self.options.nullable {
                        toks![
                            field_toks.clone(),
                            " == null ? ",
                            self.null_string.clone(),
                            " : ",
                            format,
                        ]
                    } else {
                        format
                    }
                }
            };

            let field_key = Rc::new(format!("{}=", field.name().as_ref())).quoted();

            body.push({
                let mut t = Tokens::new();
                t.push(toks!["b.append(", field_key, ");"]);
                t.push(toks!["b.append(", format, ");"]);
                t
            });
        }

        // join each field with ", "
        let mut class_appends = Tokens::new();

        class_appends.push(toks!["b.append(", name.quoted(), ");"]);
        class_appends.push(toks!["b.append(", "(".quoted(), ");",]);

        let sep = toks![Element::PushSpacing, "b.append(", ", ".quoted(), ");"];
        class_appends.push(body.join(sep));
        class_appends.push(toks!["b.append(", ")".quoted(), ");",]);

        to_string.body.push(class_appends);
        to_string.body.push(toks!["return b.toString();"]);
        to_string.body = to_string.body.join_line_spacing();

        to_string
    }

    fn add_class(
        &self,
        name: Cons<'el>,
        fields: &[JavaField<'el>],
        methods: &mut Vec<Method<'el>>,
        constructors: &mut Vec<Constructor<'el>>,
    ) -> Result<()> {
        if self.options.build_constructor {
            constructors.push(self.build_constructor(fields));
        }

        if self.options.build_hash_code {
            methods.push(self.build_hash_code(fields));
        }

        if self.options.build_equals {
            methods.push(self.build_equals(name.clone(), fields));
        }

        if self.options.build_to_string {
            methods.push(self.build_to_string(name.clone(), fields));
        }

        Ok(())
    }

    fn build_enum_constructor(&self, fields: &[Field<'el>]) -> Constructor<'el> {
        use self::Modifier::*;

        let mut c = Constructor::new();
        c.modifiers = vec![Modifier::Private];

        for field in fields {
            let mut argument = Argument::new(field.ty(), field.var());
            argument.modifiers = vec![Final];

            if !self.options.nullable {
                if let Some(non_null) = self.require_non_null(&field, &argument, "value".into()) {
                    c.body.push(non_null);
                }
            }

            c.body
                .push(toks!["this.", field.var(), " = ", argument.var(), ";",]);

            c.arguments.push(argument);
        }

        c
    }

    fn enum_from_value_method(&self, name: Cons<'el>, field: &Field<'el>) -> Method<'el> {
        use self::Modifier::*;

        let argument = Argument::new(field.ty(), field.var());

        // use naming convention that won't be generated
        let cond = match field.ty() {
            Java::Primitive { .. } => toks!["v_value.", field.var(), " == ", argument.var()],
            _ => toks!["v_value.", field.var(), ".equals(", argument.var(), ")"],
        };

        let mut return_matched = Tokens::new();

        return_matched.push(toks!["if (", cond, ") {"]);
        return_matched.nested(toks!["return v_value;"]);
        return_matched.push("}");

        let mut value_loop = Tokens::new();

        value_loop.push(toks![
            "for (final ",
            name.clone(),
            " v_value : ",
            "values()) {",
        ]);

        value_loop.nested(return_matched);
        value_loop.push("}");

        let mut from_value = Method::new("fromValue");

        from_value.modifiers = vec![Public, Static];
        from_value.returns = local(name.clone());

        let throw = toks![
            "throw new ",
            self.illegal_argument.clone(),
            "(",
            argument.var().quoted(),
            ");",
        ];

        from_value.body.push(value_loop);
        from_value.body.push(throw);
        from_value.body = from_value.body.join_line_spacing();

        from_value.arguments.push(argument);

        from_value
    }

    fn enum_to_value_method(&self, field: &Field<'el>) -> Method<'el> {
        let mut to_value = Method::new("toValue");
        to_value.returns = field.ty();
        to_value.body.push(toks!["return this.", field.var(), ";"]);
        to_value
    }

    fn enum_type_to_java(&self, ty: &'el RpEnumType) -> Result<Java<'el>> {
        use self::RpEnumType::*;

        match *ty {
            String => Ok(self.string.clone().into()),
        }
    }

    fn process_enum(&self, body: &'el RpEnumBody) -> Result<Enum<'el>> {
        let mut spec = Enum::new(body.ident.clone());

        let enum_type = self.enum_type_to_java(&body.enum_type)?;
        spec.fields.push(self.new_field_spec(&enum_type, "value"));

        for variant in &body.variants {
            let mut t = Tokens::new();

            // convert .reproto (upper-camel) convertion to Java
            let name = self.variant_naming.convert(variant.ident());
            push!(t, name, "(", variant.ordinal().quoted(), ")");

            spec.variants.append(t);
        }

        spec.constructors
            .push(self.build_enum_constructor(&spec.fields));

        let java_field = self.field(self.variant_field)?;

        let mut from_value = self.enum_from_value_method(spec.name(), &java_field.spec);
        let mut to_value = self.enum_to_value_method(&java_field.spec);

        call_codegen!(
            &self.options.enum_generators,
            EnumAdded {
                body: body,
                spec: &mut spec,
                from_value: &mut from_value,
                to_value: &mut to_value,
            }
        );

        spec.methods.push(from_value);
        spec.methods.push(to_value);
        spec.body.push_unless_empty(code(&body.codes));

        Ok(spec)
    }

    fn process_tuple(&self, body: &'el RpTupleBody) -> Result<Class<'el>> {
        let mut spec = Class::new(body.ident.clone());

        let fields = self.fields(&body.fields)?;

        self.add_class(
            spec.name(),
            &fields,
            &mut spec.methods,
            &mut spec.constructors,
        )?;

        for field in fields {
            if self.options.build_getters {
                let mut getter = field.getter();

                call_codegen!(
                    &self.options.getter_generators,
                    GetterAdded {
                        name: field.name(),
                        getter: &mut getter,
                    }
                );

                spec.methods.push(getter);
            }

            if self.options.build_setters {
                if let Some(setter) = field.setter() {
                    spec.methods.push(setter);
                }
            }

            spec.fields.push(field.spec);
        }

        spec.body.push_unless_empty(code(&body.codes));

        call_codegen!(
            &self.options.tuple_generators,
            TupleAdded { spec: &mut spec }
        );

        Ok(spec)
    }

    fn process_type(&self, body: &'el RpTypeBody) -> Result<Class<'el>> {
        let mut spec = Class::new(body.ident.clone());
        let fields = self.fields(&body.fields)?;
        let names: Vec<_> = fields.iter().map(|f| f.name.clone()).collect();

        for field in &fields {
            spec.fields.push(field.spec.clone());

            if self.options.build_getters {
                let mut getter = field.getter();

                call_codegen!(
                    &self.options.getter_generators,
                    GetterAdded {
                        name: field.name(),
                        getter: &mut getter,
                    }
                );

                spec.methods.push(getter);
            }

            if self.options.build_setters {
                if let Some(setter) = field.setter() {
                    spec.methods.push(setter);
                }
            }
        }

        spec.body.push_unless_empty(code(&body.codes));

        self.add_class(
            spec.name(),
            &fields,
            &mut spec.methods,
            &mut spec.constructors,
        )?;

        for generator in &self.options.class_generators {
            generator.generate(ClassAdded {
                names: &names,
                spec: &mut spec,
            })?;
        }

        Ok(spec)
    }

    fn process_interface(
        &self,
        depth: usize,
        body: &'el RpInterfaceBody,
    ) -> Result<Interface<'el>> {
        use self::Modifier::*;
        let mut spec = Interface::new(body.ident.clone());
        let interface_fields = self.fields(&body.fields)?;

        for field in &interface_fields {
            let mut m = field.getter_without_body();
            m.modifiers = vec![];
            spec.methods.push(m);
        }

        spec.body.push_unless_empty(code(&body.codes));

        body.sub_types.iter().for_each_loc(|sub_type| {
            let mut class = Class::new(sub_type.ident.clone());
            class.modifiers = vec![Public, Static];

            let sub_type_fields = self.fields(&sub_type.fields)?;

            class.body.push_unless_empty(code(&sub_type.codes));

            class.implements = vec![local(spec.name())];

            // override methods for interface fields.
            for field in &interface_fields {
                if self.options.build_getters {
                    let mut getter = field.getter();
                    getter.annotation(Override);

                    call_codegen!(
                        &self.options.getter_generators,
                        GetterAdded {
                            name: field.name(),
                            getter: &mut getter,
                        }
                    );

                    class.methods.push(getter);
                }

                if self.options.build_setters {
                    if let Some(mut setter) = field.setter() {
                        setter.annotation(Override);
                        class.methods.push(setter);
                    }
                }
            }

            for field in &sub_type_fields {
                if self.options.build_getters {
                    let mut getter = field.getter();

                    call_codegen!(
                        &self.options.getter_generators,
                        GetterAdded {
                            name: field.name(),
                            getter: &mut getter,
                        }
                    );

                    class.methods.push(getter);
                }

                if self.options.build_setters {
                    if let Some(setter) = field.setter() {
                        class.methods.push(setter);
                    }
                }
            }

            let mut fields = interface_fields.to_vec();
            fields.extend(sub_type_fields);
            let names: Vec<_> = fields.iter().map(|f| f.name.clone()).collect();

            class.fields.extend(fields.iter().map(|f| f.spec.clone()));

            self.add_class(
                class.name(),
                &fields,
                &mut class.methods,
                &mut class.constructors,
            )?;

            for generator in &self.options.class_generators {
                generator.generate(ClassAdded {
                    names: &names,
                    spec: &mut class,
                })?;
            }

            // Process sub-type declarations.
            for d in &sub_type.decls {
                self.process_decl(d, depth + 1, &mut class.body)?;
            }

            spec.body.push(class);
            Ok(()) as Result<()>
        })?;

        for generator in &self.options.interface_generators {
            generator.generate(InterfaceAdded {
                body: body,
                spec: &mut spec,
            })?;
        }

        Ok(spec)
    }

    fn process_service(&self, body: &'el RpServiceBody) -> Result<Interface<'el>> {
        let mut spec = Interface::new(body.ident.as_str());

        let mut extra: Vec<EndpointExtra> = Vec::new();

        for endpoint in &body.endpoints {
            let name = self.to_lower_camel.convert(endpoint.safe_ident());

            let response_ty = if let Some(res) = endpoint.response.as_ref() {
                let ty = self.utils.into_java_type(res.ty())?;
                self.observer.with_arguments(vec![ty])
            } else {
                self.observer.with_arguments(vec![self.void.clone()])
            };

            let request_ty = if let Some(req) = endpoint.request.as_ref() {
                let ty = self.utils.into_java_type(req.channel.ty())?;
                self.observer.with_arguments(vec![ty])
            } else {
                self.observer.with_arguments(vec![self.void.clone()])
            };

            let mut arguments = Vec::new();

            for arg in &endpoint.arguments {
                let ty = self.utils.into_java_type(arg.channel.ty())?;
                arguments.push(Argument::new(ty, arg.safe_ident()));
            }

            extra.push(EndpointExtra {
                name: Rc::new(name).into(),
                response_ty: response_ty,
                request_ty: request_ty,
                arguments: arguments,
            });
        }

        if !self.options.suppress_service_methods {
            for (endpoint, extra) in body.endpoints.iter().zip(extra.iter()) {
                let EndpointExtra {
                    ref name,
                    ref response_ty,
                    ref arguments,
                    ..
                } = *extra;

                let mut method = Method::new(name.clone());

                if !endpoint.comment.is_empty() {
                    method.comments.push("<pre>".into());
                    method
                        .comments
                        .extend(endpoint.comment.iter().cloned().map(Into::into));
                    method.comments.push("</pre>".into());
                }

                method.modifiers = vec![];
                method.arguments.extend(arguments.iter().cloned());

                method.returns = response_ty.clone();
                spec.methods.push(method);
            }
        }

        for generator in &self.options.service_generators {
            generator.generate(ServiceAdded {
                compiler: self,
                body: body,
                extra: &extra,
                spec: &mut spec,
            })?;
        }

        Ok(spec)
    }

    /// Convert a single field to `JavaField`, without comments.
    fn field(&self, field: &'el RpField) -> Result<JavaField<'el>> {
        let java_value_type = self.utils.into_java_type(&field.ty)?;

        let java_type = if field.is_optional() {
            optional(
                java_value_type.clone(),
                self.optional.with_arguments(vec![java_value_type.clone()]),
            )
        } else {
            java_value_type
        };

        let ident = Rc::new(self.to_lower_camel.convert(field.safe_ident()));
        let field_accessor = Rc::new(self.to_upper_camel.convert(field.ident()));

        let mut spec = Field::new(java_type, ident.clone());

        if !field.comment.is_empty() {
            spec.comments.push("<pre>".into());
            spec.comments
                .extend(field.comment.iter().map(|c| Cons::from(c.as_str())));
            spec.comments.push("</pre>".into());
        }

        Ok(JavaField {
            name: Cons::from(field.name()),
            ident: ident,
            field_accessor: field_accessor,
            spec: spec,
        })
    }

    /// Convert fields to `JavaField`.
    fn fields(&self, fields: &'el [Loc<RpField>]) -> Result<Vec<JavaField<'el>>> {
        let mut out = Vec::new();

        fields.for_each_loc(|field| {
            out.push(self.field(field)?);
            Ok(()) as Result<()>
        })?;

        Ok(out)
    }

    pub fn process_decl(
        &self,
        decl: &'el RpDecl,
        depth: usize,
        container: &mut Tokens<'el, Java<'el>>,
    ) -> Result<()> {
        match *decl {
            RpDecl::Interface(ref interface) => {
                let mut spec = self.process_interface(depth + 1, interface)?;

                for d in &interface.decls {
                    self.process_decl(d, depth + 1, &mut spec.body)?;
                }

                container.push(spec);
            }
            RpDecl::Type(ref ty) => {
                let mut spec = self.process_type(ty)?;

                // Inner classes should be static.
                if depth > 0 {
                    spec.modifiers.push(Modifier::Static);
                }

                for d in &ty.decls {
                    self.process_decl(d, depth + 1, &mut spec.body)?;
                }

                container.push(spec);
            }
            RpDecl::Tuple(ref ty) => {
                let mut spec = self.process_tuple(ty)?;

                // Inner classes should be static.
                if depth > 0 {
                    spec.modifiers.push(Modifier::Static);
                }

                for d in &ty.decls {
                    self.process_decl(d, depth + 1, &mut spec.body)?;
                }

                container.push(spec);
            }
            RpDecl::Enum(ref ty) => {
                let mut spec = self.process_enum(ty)?;

                // Inner classes should be static.
                if depth > 0 {
                    spec.modifiers.push(Modifier::Static);
                }

                container.push(spec);
            }
            RpDecl::Service(ref ty) => {
                let mut spec = self.process_service(ty)?;

                // Inner classes should be static.
                if depth > 0 {
                    spec.modifiers.push(Modifier::Static);
                }

                for d in &ty.decls {
                    self.process_decl(d, depth + 1, &mut spec.body)?;
                }

                container.push(spec);
            }
        }

        Ok(())
    }
}

impl<'el> Converter<'el> for Compiler<'el> {
    type Custom = Java<'el>;

    fn convert_type(&self, name: &RpName) -> Result<Tokens<'el, Self::Custom>> {
        Ok(toks![self.utils.convert_type_id(name)?])
    }
}
