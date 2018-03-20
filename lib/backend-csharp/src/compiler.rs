//! C# backend for reproto

use Options;
use backend::Converter;
use codegen::{ClassAdded, EndpointExtra, EnumAdded, InterfaceAdded, ServiceAdded, TupleAdded,
              TypeField, TypeFieldAdded};
use core::{ForEachLoc, Handle, Loc, RpContext, RpDecl, RpEnumBody, RpField, RpInterfaceBody,
           RpName, RpServiceBody, RpSubTypeStrategy, RpTupleBody, RpTypeBody, WithPos};
use core::errors::*;
use csharp_field::CsharpField;
use csharp_file::CsharpFile;
use genco::{Cons, Csharp, Element, Quoted, Tokens};
use genco::csharp::{Argument, BOOLEAN, Class, Constructor, Enum, Field, INT32, Method, Modifier,
                    local, optional, using};
use naming::{self, Naming};
use processor::Processor;
use std::rc::Rc;
use trans::Environment;
use utils::Utils;

pub struct Compiler {
    env: Rc<Environment>,
    utils: Rc<Utils>,
    options: Options,
    to_upper_camel: naming::ToUpperCamel,
    to_lower_camel: naming::ToLowerCamel,
    variant_naming: naming::ToUpperSnake,
    string_builder: Csharp<'static>,
    object: Csharp<'static>,
    string: Csharp<'static>,
    task: Csharp<'static>,
}

impl Processor for Compiler {}

impl Compiler {
    pub fn new(env: &Rc<Environment>, utils: &Rc<Utils>, options: Options) -> Compiler {
        Compiler {
            env: Rc::clone(env),
            utils: Rc::clone(utils),
            options: options,
            to_upper_camel: naming::to_upper_camel(),
            to_lower_camel: naming::to_lower_camel(),
            variant_naming: naming::to_upper_snake(),
            string_builder: using("System.Text", "StringBuilder"),
            object: using("System", "Object"),
            string: using("System", "String"),
            task: using("System.Threading.Tasks", "Task"),
        }
    }

    pub fn compile(&self, handle: &Handle) -> Result<()> {
        for generator in &self.options.root_generators {
            generator.generate(handle)?;
        }

        for decl in self.env.toplevel_decl_iter() {
            self.compile_decl(handle, decl).with_pos(decl.pos())?;
        }

        Ok(())
    }

    fn compile_decl(&self, handle: &Handle, decl: &RpDecl) -> Result<()> {
        let package_name = self.csharp_package(&decl.name().package).parts.join(".");

        CsharpFile::new(package_name.as_str(), decl.ident(), |out| {
            self.process_decl(decl, 0usize, out)
        }).process(handle)
    }

    fn build_constructor<'a, 'el>(&self, fields: &[CsharpField<'el>]) -> Constructor<'el> {
        let mut c = Constructor::new();

        for field in fields {
            let spec = &field.spec;

            let argument = Argument::new(spec.ty(), spec.var());

            c.arguments.push(argument.clone());

            c.body.push(
                toks!["this.", field.spec.var(), " = ", argument.var(), ";",],
            );
        }

        c
    }

    fn build_hash_code<'el>(&self, fields: &[CsharpField<'el>]) -> Method<'el> {
        let mut hash_code = Method::new("GetHashCode");

        hash_code.modifiers = vec![Modifier::Public, Modifier::Override];
        hash_code.returns = INT32;

        hash_code.body.push("Int32 result = 1;");

        for field in fields {
            let field = &field.spec;

            let field_toks = toks!["this.", field.var()];

            let value = toks![field_toks.clone(), ".GetHashCode()"];

            hash_code.body.push(
                toks!["result = result * 31 + ", value, ";"],
            );
        }

        hash_code.body.push("return result;");
        hash_code
    }

    fn false_cond<'el>(&self, cond: Tokens<'el, Csharp<'el>>) -> Tokens<'el, Csharp<'el>> {
        let mut t = Tokens::new();

        t.push(toks!["if (", cond, ") {"]);
        t.nested("return false;");
        t.push("}");

        t
    }

    fn build_equals<'el>(&self, name: Cons<'el>, fields: &[CsharpField<'el>]) -> Method<'el> {
        let argument = Argument::new(self.object.clone(), "other");

        let mut m = Method::new("Equals");

        m.modifiers = vec![Modifier::Public, Modifier::Override];
        m.returns = BOOLEAN;
        m.arguments.push(argument.clone());

        // cast argument.
        m.body.push({
            let mut t = Tokens::new();

            t.push(toks![
                name.clone(),
                " o = ",
                argument.var(),
                " as ",
                name.clone(),
                ";",
            ]);

            t
        });

        // check if argument is null.
        m.body.push({
            let mut t = Tokens::new();

            t.push(toks!["if (o == null) {"]);
            t.nested("return false;");
            t.push("}");

            t
        });

        for field in fields {
            let field = &field.spec;
            let this = toks!["this.", field.var()];
            let o = toks!["o.", field.var()];

            let cond = match field.ty() {
                // Simple type.
                Csharp::Simple { .. } => self.false_cond(toks![this.clone(), " != ", o.clone()]),
                // Type is wrapped in a Nullable<T>, and always implements Equals.
                Csharp::Optional(ref inner) if !inner.is_nullable() => {
                    self.false_cond(toks!["!", this.clone(), ".Equals(", o.clone(), ")"])
                }
                // Optional type.
                Csharp::Optional(_) => {
                    let mut t = Tokens::new();

                    t.push(toks!["if (", this.clone(), " == null) {"]);
                    t.nested(self.false_cond(toks![o.clone(), " != null"]));
                    t.push(toks!["} else {"]);
                    t.nested(self.false_cond(
                        toks!["!", this.clone(), ".Equals(", o.clone(), ")"],
                    ));
                    t.push("}");

                    t
                }
                _ => self.false_cond(toks!["!", this.clone(), ".Equals(", o.clone(), ")"]),
            };

            m.body.push(cond);
        }

        m.body.push("return true;");
        m.body = m.body.join_line_spacing();

        m
    }

    fn build_to_string<'el>(&self, name: Cons<'el>, fields: &[CsharpField<'el>]) -> Method<'el> {
        let mut to_string = Method::new("ToString");

        to_string.modifiers = vec![Modifier::Public, Modifier::Override];
        to_string.returns = self.string.clone();

        to_string.body.push(toks![
            self.string_builder.clone(),
            " b = new ",
            self.string_builder.clone(),
            "();",
        ]);

        let mut body = Tokens::new();

        for field in fields {
            let field_toks = toks!["this.", field.spec.var()];
            let field_key = Rc::new(format!("{}=", field.name().as_ref())).quoted();

            body.push({
                let mut t = Tokens::new();
                t.push(toks!["b.Append(", field_key, ");"]);
                t.push(toks!["b.Append(", field_toks.clone(), ");"]);
                t
            });
        }

        // join each field with ", "
        let mut class_appends = Tokens::new();

        class_appends.push(toks!["b.Append(", name.quoted(), ");"]);
        class_appends.push(toks!["b.Append(", "(".quoted(), ");",]);

        let sep = toks![Element::PushSpacing, "b.Append(", ", ".quoted(), ");"];
        class_appends.push(body.join(sep));
        class_appends.push(toks!["b.Append(", ")".quoted(), ");",]);

        to_string.body.push(class_appends);
        to_string.body.push(toks!["return b.ToString();"]);
        to_string.body = to_string.body.join_line_spacing();

        to_string
    }

    fn add_class<'el>(
        &self,
        name: Cons<'el>,
        fields: &[CsharpField<'el>],
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

    fn process_enum<'el>(&self, body: &'el RpEnumBody) -> Result<Enum<'el>> {
        let mut spec = Enum::new(body.ident.clone());

        let mut names = Vec::new();

        for variant in &body.variants {
            let name = Rc::new(self.variant_naming.convert(variant.ident()));
            names.push(variant.ordinal().into());
            spec.variants.append(toks![name]);
        }

        for generator in &self.options.enum_generators {
            generator.generate(EnumAdded {
                body: body,
                spec: &mut spec,
                names: &names,
            })?;
        }

        /*spec.body.push_unless_empty(code!(&body.codes, RpContext::Csharp));*/

        Ok(spec)
    }

    fn process_tuple<'el>(&self, body: &'el RpTupleBody) -> Result<Class<'el>> {
        let mut spec = Class::new(body.ident.clone());

        let fields = self.fields(&body.fields)?;

        self.add_class(
            spec.name(),
            &fields,
            &mut spec.methods,
            &mut spec.constructors,
        )?;

        for field in fields {
            spec.fields.push(field.spec);
        }

        spec.body.push_unless_empty(
            code!(&body.codes, RpContext::Csharp),
        );

        for generator in &self.options.tuple_generators {
            generator.generate(TupleAdded { spec: &mut spec })?;
        }

        Ok(spec)
    }

    fn process_type<'el>(&self, body: &'el RpTypeBody) -> Result<Class<'el>> {
        let mut spec = Class::new(body.ident.clone());
        let fields = self.fields(&body.fields)?;
        let names: Vec<_> = fields.iter().map(|f| f.name.clone()).collect();

        for field in &fields {
            spec.fields.push(field.spec.clone());
        }

        spec.body.push_unless_empty(
            code!(&body.codes, RpContext::Csharp),
        );

        self.add_class(
            spec.name(),
            &fields,
            &mut spec.methods,
            &mut spec.constructors,
        )?;

        for generator in &self.options.class_generators {
            generator.generate(ClassAdded {
                type_field: None,
                names: &names,
                spec: &mut spec,
                fields: &fields,
            })?;
        }

        Ok(spec)
    }

    fn process_interface<'el>(
        &self,
        depth: usize,
        body: &'el RpInterfaceBody,
    ) -> Result<Class<'el>> {
        let mut spec = Class::new(body.ident.clone());
        spec.modifiers = vec![Modifier::Abstract, Modifier::Public];
        let interface_fields = self.fields(&body.fields)?;

        let type_field = match body.sub_type_strategy {
            RpSubTypeStrategy::Tagged { ref tag, .. } => {
                let mut f = Field::new(self.string.clone(), Cons::from("TypeField"));

                let mut block = Tokens::new();
                block.push("get;");
                f.block = Some(block);

                let tag = Cons::from(Rc::new(tag.clone()));

                for generator in &self.options.type_field_generators {
                    generator.generate(TypeFieldAdded {
                        tag: tag.clone(),
                        field: &mut f,
                    })?;
                }

                spec.fields.push(f.clone());

                Some(TypeField {
                    field: f,
                    tag: tag.clone(),
                })
            }
        };

        // Setup the constructor that takes the type field.
        spec.constructors.push({
            let mut c = Constructor::new();

            if let Some(&TypeField { ref field, .. }) = type_field.as_ref() {
                c.arguments.push(Argument::new(field.ty(), field.var()));
                c.body.push(
                    toks!["this.", field.var(), " = ", field.var(), ";"],
                );
            }

            c
        });

        spec.body.push_unless_empty(
            code!(&body.codes, RpContext::Csharp),
        );

        body.sub_types.iter().for_each_loc(|sub_type| {
            let mut class = Class::new(sub_type.ident.clone());
            class.modifiers = vec![Modifier::Public];

            let sub_type_fields = self.fields(&sub_type.fields)?;

            class.body.push_unless_empty(
                code!(&sub_type.codes, RpContext::Csharp),
            );

            class.implements = vec![local(spec.name())];

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
                    type_field: type_field.clone(),
                    names: &names,
                    spec: &mut class,
                    fields: &fields,
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

    fn process_service<'el>(&self, body: &'el RpServiceBody) -> Result<Class<'el>> {
        let mut spec = Class::new(body.ident.as_str());
        spec.modifiers = vec![Modifier::Abstract, Modifier::Public];

        let mut extra: Vec<EndpointExtra> = Vec::new();

        for endpoint in &body.endpoints {
            let name = self.to_lower_camel.convert(endpoint.safe_ident());

            let response_ty = if let Some(res) = endpoint.response.as_ref() {
                let ty = self.utils.into_csharp_type(res.ty())?;
                self.task.with_arguments(vec![ty])
            } else {
                Csharp::Void
            };

            let mut arguments = Vec::new();

            for arg in &endpoint.arguments {
                let ty = self.utils.into_csharp_type(arg.channel.ty())?;
                arguments.push(Argument::new(ty, arg.safe_ident()));
            }

            extra.push(EndpointExtra {
                name: Rc::new(name).into(),
                response_ty: response_ty,
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

                let name = Rc::new(self.to_upper_camel.convert(name));
                let mut method = Method::new(name);
                method.modifiers = vec![Modifier::Abstract, Modifier::Public];

                if !endpoint.comment.is_empty() {
                    method.comments.push("<summary>".into());
                    method.comments.extend(
                        endpoint.comment.iter().cloned().map(
                            Into::into,
                        ),
                    );
                    method.comments.push("</summary>".into());
                }

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

    /// Convert a single field to `CsharpField`, without comments.
    fn field<'el>(&self, field: &RpField) -> Result<CsharpField<'el>> {
        let value_type = self.utils.into_csharp_type(&field.ty)?;

        let csharp_ty = if field.is_optional() {
            optional(value_type)
        } else {
            value_type.into()
        };

        let ident = Rc::new(self.to_lower_camel.convert(field.safe_ident()));

        let mut spec = Field::new(csharp_ty, ident.clone());
        spec.modifiers = vec![Modifier::Public];

        if !field.comment.is_empty() {
            spec.comments.push("<summary>".into());
            spec.comments.extend(field.comment.iter().map(
                |c| Cons::from(Rc::new(c.to_string())),
            ));
            spec.comments.push("</summary>".into());
        }

        let mut block = Tokens::new();

        if self.options.build_getters {
            block.push("get;");
        }

        if !block.is_empty() {
            spec.block = Some(block);
        }

        Ok(CsharpField {
            name: Rc::new(field.name().to_string()).into(),
            ident: ident,
            spec: spec,
            optional: field.is_optional(),
        })
    }

    /// Convert fields to `CsharpField`.
    fn fields<'el>(&self, fields: &'el [Loc<RpField>]) -> Result<Vec<CsharpField<'el>>> {
        let mut out = Vec::new();

        fields.for_each_loc(|field| {
            out.push(self.field(field)?);
            Ok(()) as Result<()>
        })?;

        Ok(out)
    }

    pub fn process_decl<'el>(
        &self,
        decl: &'el RpDecl,
        depth: usize,
        container: &mut Tokens<'el, Csharp<'el>>,
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

                for d in &ty.decls {
                    self.process_decl(d, depth + 1, &mut spec.body)?;
                }

                container.push(spec);
            }
            RpDecl::Tuple(ref ty) => {
                let mut spec = self.process_tuple(ty)?;

                for d in &ty.decls {
                    self.process_decl(d, depth + 1, &mut spec.body)?;
                }

                container.push(spec);
            }
            RpDecl::Enum(ref ty) => {
                container.push(self.process_enum(ty)?);
            }
            RpDecl::Service(ref ty) => {
                let mut spec = self.process_service(ty)?;

                for d in &ty.decls {
                    self.process_decl(d, depth + 1, &mut spec.body)?;
                }

                container.push(spec);
            }
        }

        Ok(())
    }
}

impl<'el> Converter<'el> for Compiler {
    type Custom = Csharp<'el>;

    fn convert_type(&self, name: &RpName) -> Result<Tokens<'el, Self::Custom>> {
        Ok(toks![self.utils.convert_type_id(name)?])
    }
}
