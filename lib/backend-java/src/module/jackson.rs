//! Module that adds fasterxml annotations to generated classes.

use codegen::{ClassAdded, ClassCodegen, Configure, EnumAdded, EnumCodegen, GetterAdded,
              GetterCodegen, InterfaceAdded, InterfaceCodegen, TupleAdded, TupleCodegen};
use core::RpSubTypeStrategy;
use core::errors::*;
use genco::java::{imported, local, Argument, Class, Field, Modifier, DOUBLE, FLOAT, INTEGER, LONG,
                  SHORT};
use genco::{Cons, Element, IntoTokens, Java, Quoted, Tokens};
use std::rc::Rc;

pub struct Module;

impl Module {
    pub fn initialize(self, e: Configure) {
        let jackson = Rc::new(Jackson::new());
        e.options.getter_generators.push(Box::new(jackson.clone()));
        e.options.class_generators.push(Box::new(jackson.clone()));
        e.options.tuple_generators.push(Box::new(jackson.clone()));
        e.options
            .interface_generators
            .push(Box::new(jackson.clone()));
        e.options.enum_generators.push(Box::new(jackson.clone()));
    }
}

/// @JsonSubTypes.Type annotation
struct SubTypesType<'a, 'el>(&'a Jackson, Tokens<'el, Java<'el>>);

impl<'a, 'el> IntoTokens<'el, Java<'el>> for SubTypesType<'a, 'el> {
    fn into_tokens(self) -> Tokens<'el, Java<'el>> {
        toks!["@", self.0.sub_type.clone(), "(", self.1.join(", "), ")"]
    }
}

/// @JsonSubTypes annotation
struct SubTypes<'a, 'el>(&'a Jackson, Tokens<'el, Java<'el>>);

impl<'a, 'el> IntoTokens<'el, Java<'el>> for SubTypes<'a, 'el> {
    fn into_tokens(self) -> Tokens<'el, Java<'el>> {
        let mut out: Tokens<Java> = Tokens::new();
        out.append("@");
        out.append(self.0.sub_types.clone());
        out.append("({");

        if !self.1.is_empty() {
            out.nested(self.1.join(toks![",", Element::PushSpacing]))
        }

        out.append("})");
        out
    }
}

struct TypeInfo<'a, 'el>(&'a Jackson, Tokens<'el, Java<'el>>);

impl<'a, 'el> IntoTokens<'el, Java<'el>> for TypeInfo<'a, 'el> {
    fn into_tokens(self) -> Tokens<'el, Java<'el>> {
        toks!["@", self.0.type_info.clone(), "(", self.1.join(", "), ")"]
    }
}

struct JsonFormat;

impl<'el> IntoTokens<'el, Java<'el>> for JsonFormat {
    fn into_tokens(self) -> Tokens<'el, Java<'el>> {
        let json_format = imported("com.fasterxml.jackson.annotation", "JsonFormat");

        let mut args = Tokens::new();
        args.append(toks!["shape = ", json_format.clone(), ".Shape.STRING"]);

        toks!["@", json_format, "(", args.join(", "), ")"]
    }
}

struct JsonProperty<'el>(Cons<'el>);

impl<'el> IntoTokens<'el, Java<'el>> for JsonProperty<'el> {
    fn into_tokens(self) -> Tokens<'el, Java<'el>> {
        let json_property = imported("com.fasterxml.jackson.annotation", "JsonProperty");
        toks!["@", json_property, "(", self.0.quoted(), ")"]
    }
}

pub struct Jackson {
    override_: Java<'static>,
    creator: Java<'static>,
    value: Java<'static>,
    sub_types: Java<'static>,
    sub_type: Java<'static>,
    type_info: Java<'static>,
    serialize: Java<'static>,
    deserialize: Java<'static>,
    deserializer: Java<'static>,
    serializer: Java<'static>,
    generator: Java<'static>,
    serializer_provider: Java<'static>,
    parser: Java<'static>,
    deserialization_context: Java<'static>,
    type_reference: Java<'static>,
    token: Java<'static>,
    string: Java<'static>,
    instant: Java<'static>,
    io_exception: Java<'static>,
}

impl Jackson {
    pub fn new() -> Jackson {
        Jackson {
            override_: imported("java.lang", "Override"),
            creator: imported("com.fasterxml.jackson.annotation", "JsonCreator"),
            value: imported("com.fasterxml.jackson.annotation", "JsonValue"),
            sub_types: imported("com.fasterxml.jackson.annotation", "JsonSubTypes"),
            sub_type: imported("com.fasterxml.jackson.annotation", "JsonSubTypes").path("Type"),
            type_info: imported("com.fasterxml.jackson.annotation", "JsonTypeInfo"),
            serialize: imported("com.fasterxml.jackson.databind.annotation", "JsonSerialize"),
            deserialize: imported(
                "com.fasterxml.jackson.databind.annotation",
                "JsonDeserialize",
            ),
            serializer: imported("com.fasterxml.jackson.databind", "JsonSerializer"),
            deserializer: imported("com.fasterxml.jackson.databind", "JsonDeserializer"),
            generator: imported("com.fasterxml.jackson.core", "JsonGenerator"),
            serializer_provider: imported("com.fasterxml.jackson.databind", "SerializerProvider"),
            parser: imported("com.fasterxml.jackson.core", "JsonParser"),
            deserialization_context: imported(
                "com.fasterxml.jackson.databind",
                "DeserializationContext",
            ),
            type_reference: imported("com.fasterxml.jackson.core.type", "TypeReference"),
            token: imported("com.fasterxml.jackson.core", "JsonToken"),
            string: imported("java.lang", "String"),
            instant: imported("java.time", "Instant"),
            io_exception: imported("java.io", "IOException"),
        }
    }

    /// RpName serialize implementation for tuples.
    fn tuple_serializer<'el>(
        &self,
        name: Cons<'el>,
        fields: &mut [Field<'el>],
    ) -> Result<Class<'el>> {
        use self::Modifier::*;

        let ty = local(name.clone());

        let value = Argument::new(ty.clone(), "value");
        let jgen = Argument::new(self.generator.clone(), "jgen");
        let provider = Argument::new(self.serializer_provider.clone(), "provider");

        let mut serialize = Tokens::new();

        serialize.push(toks!("@", self.override_.clone()));
        serialize.push(toks![
            "public void serialize(",
            toks![
                value.into_tokens(),
                jgen.into_tokens(),
                provider.into_tokens()
            ].join(", "),
            ") throws ",
            self.io_exception.clone(),
            " {",
        ]);

        serialize.nested({
            let mut t = Tokens::new();
            t.push(toks!["jgen.writeStartArray();"]);

            for field in fields {
                let access = toks!["value.", field.var()];

                let write = match field.ty() {
                    SHORT | LONG | INTEGER | FLOAT | DOUBLE => {
                        toks!["writeNumber(", access.clone(), ")"]
                    }
                    Java::Primitive { .. } => {
                        return Err("cannot serialize type".into());
                    }
                    class @ Java::Class { .. } => {
                        if class == self.string {
                            toks!["writeString(", access.clone(), ")"]
                        } else {
                            toks!["writeObject(", access.clone(), ")"]
                        }
                    }
                    _ => toks!["writeObject(", access.clone(), ")"],
                };

                t.push(toks!["jgen.", write, ";"]);
            }

            t.push(toks!["jgen.writeEndArray();"]);

            t
        });

        serialize.push("}");

        let mut class = Class::new("Serializer");
        class.modifiers.push(Static);
        class.extends = Some(self.serializer.with_arguments(vec![ty.clone()]));
        class.body.push(serialize);
        Ok(class)
    }

    fn deserialize_method_for_type<'el, A>(
        &self,
        ty: Java<'el>,
        parser: A,
    ) -> Result<(
        Option<(Tokens<'el, Java<'el>>, &'el str)>,
        Tokens<'el, Java<'el>>,
    )>
    where
        A: Into<Tokens<'el, Java<'el>>>,
    {
        let p = parser.into();

        let (token, reader) = match ty {
            java @ Java::Primitive { .. } => {
                let test = toks!["!", p.clone(), ".nextToken().isNumeric()"];

                match java {
                    SHORT => (
                        Some((test, "VALUE_NUMBER_INT")),
                        toks![p, ".getShortValue()"],
                    ),
                    LONG => (
                        Some((test, "VALUE_NUMBER_INT")),
                        toks![p, ".getLongValue()"],
                    ),
                    INTEGER => (Some((test, "VALUE_NUMBER_INT")), toks![p, ".getIntValue()"]),
                    FLOAT => (
                        Some((test, "VALUE_NUMBER_FLOAT")),
                        toks![p, ".getFloatValue()"],
                    ),
                    DOUBLE => (
                        Some((test, "VALUE_NUMBER_FLOAT")),
                        toks![p, ".getDoubleValue()"],
                    ),
                    _ => {
                        return Err("unsupported type".into());
                    }
                }
            }
            class @ Java::Class { .. } => {
                if class == self.string {
                    let test = toks![
                        p.clone(),
                        ".nextToken() != ",
                        self.token.clone(),
                        ".VALUE_STRING",
                    ];
                    let token = Some((test, "VALUE_STRING"));
                    (token, toks![p, ".getText()"])
                } else {
                    let is_empty = class.arguments().map(|a| a.is_empty()).unwrap_or(true);

                    let argument = if is_empty {
                        toks![class, ".class"]
                    } else {
                        toks![
                            "new ",
                            self.type_reference.with_arguments(vec![class]),
                            "(){}",
                        ]
                    };

                    (None, toks![p, ".readValueAs(", argument, ")"])
                }
            }
            _ => {
                return Err("unsupported type".into());
            }
        };

        Ok((token, reader))
    }

    fn wrong_token<'el, C, P, T>(&self, ctxt: C, parser: P, token: T) -> Tokens<'el, Java<'el>>
    where
        C: Into<Tokens<'el, Java<'el>>>,
        P: Into<Tokens<'el, Java<'el>>>,
        T: Into<Tokens<'el, Java<'el>>>,
    {
        let mut arguments = Tokens::new();

        arguments.append(parser.into());
        arguments.append(toks![self.token.clone(), ".", token.into()]);
        arguments.append("null");

        toks![
            "throw ",
            ctxt.into(),
            ".wrongTokenException(",
            arguments.join(", "),
            ");",
        ]
    }

    /// RpName deserialize implementation for tuples.
    fn tuple_deserializer<'el>(
        &self,
        name: Cons<'el>,
        fields: &mut [Field<'el>],
    ) -> Result<Class<'el>> {
        use self::Modifier::*;

        let ty = local(name.clone());

        let parser = toks!("final ", self.parser.clone(), " parser");
        let ctxt = toks!("final ", self.deserialization_context.clone(), " ctxt");

        let mut deserialize = Tokens::new();

        deserialize.push(toks!("@", self.override_.clone()));
        deserialize.push(toks![
            "public ",
            ty.clone(),
            " deserialize(",
            toks![parser, ctxt].join(", "),
            ") throws ",
            self.io_exception.clone(),
            " {",
        ]);

        deserialize.nested({
            let mut body = Tokens::new();
            let current_token = toks!["parser.getCurrentToken()"];

            let mut start_array = Tokens::new();
            start_array.push(toks![
                "if (",
                current_token,
                " != ",
                self.token.clone(),
                ".START_ARRAY) {",
            ]);
            start_array.nested(self.wrong_token("ctxt", "parser", "START_ARRAY"));
            start_array.push("}");
            body.push(start_array);

            let mut arguments = Tokens::new();

            for field in fields {
                let (token, reader) = self.deserialize_method_for_type(field.ty(), "parser")?;

                if let Some((test, expected)) = token {
                    let mut field_check = Tokens::new();
                    field_check.push(toks!["if (", test, ") {"]);
                    field_check.nested(self.wrong_token("ctxt", "parser", expected));
                    field_check.push("}");
                    body.push(field_check);
                } else {
                    body.push("parser.nextToken();");
                }

                let variable = toks!["v_", field.var()];
                let assign = toks![
                    "final ",
                    field.ty(),
                    " ",
                    variable.clone(),
                    " = ",
                    reader,
                    ";",
                ];
                body.push(assign);
                arguments.append(variable);
            }

            let mut end_array = Tokens::new();
            end_array.push(toks![
                "if (parser.nextToken() != ",
                self.token.clone(),
                ".END_ARRAY) {",
            ]);
            end_array.nested(self.wrong_token("ctxt", "parser", "END_ARRAY"));
            end_array.push("}");
            body.push(end_array);

            body.push(toks![
                "return new ",
                ty.clone(),
                "(",
                arguments.join(", "),
                ");",
            ]);

            body.join_line_spacing()
        });

        deserialize.push("}");

        Ok({
            let mut deserializer = Class::new("Deserializer");
            deserializer.modifiers.push(Static);
            deserializer.extends = Some(self.deserializer.with_arguments(vec![ty.clone()]));
            deserializer.body.push(deserialize);
            deserializer
        })
    }

    fn add_class_annotations<'a>(&self, names: &[Cons<'a>], spec: &mut Class<'a>) -> Result<()> {
        // Annotate all constructors.
        for c in &mut spec.constructors {
            c.annotation(toks!["@", self.creator.clone()]);

            for (argument, name) in c.arguments.iter_mut().zip(names.iter()) {
                argument.annotation(JsonProperty(name.clone()));
            }
        }

        // Also add field annotations, since they are used during serialization!
        for (field, name) in spec.fields.iter_mut().zip(names.iter()) {
            field.annotation(JsonProperty(name.clone()));

            if field.ty().as_value() == self.instant {
                field.annotation(JsonFormat);
            }
        }

        Ok(())
    }

    fn add_tuple_serialization(&self, spec: &mut Class) -> Result<()> {
        let serializer = self.tuple_serializer(spec.name(), &mut spec.fields)?;

        let serializer_type = Rc::new(format!(
            "{}.{}",
            spec.name().as_ref(),
            serializer.name().as_ref()
        ));

        spec.annotation(toks![
            "@",
            self.serialize.clone(),
            "(using = ",
            serializer_type,
            ".class)",
        ]);

        spec.body.push(serializer);

        let deserializer = self.tuple_deserializer(spec.name(), &mut spec.fields)?;

        let deserializer_type = Rc::new(format!(
            "{}.{}",
            spec.name().as_ref(),
            deserializer.name().as_ref()
        ));

        let deserialize = toks![
            "@",
            self.deserialize.clone(),
            "(using = ",
            deserializer_type,
            ".class)",
        ];

        spec.annotation(deserialize);
        spec.body.push(deserializer);
        Ok(())
    }
}

impl GetterCodegen for Jackson {
    fn generate(&self, e: GetterAdded) -> Result<()> {
        e.getter.annotation(JsonProperty(e.name.clone()));
        Ok(())
    }
}

impl ClassCodegen for Jackson {
    fn generate(&self, e: ClassAdded) -> Result<()> {
        self.add_class_annotations(&e.names, e.spec)
    }
}

impl TupleCodegen for Jackson {
    fn generate(&self, e: TupleAdded) -> Result<()> {
        self.add_tuple_serialization(e.spec)
    }
}

impl EnumCodegen for Jackson {
    fn generate(&self, e: EnumAdded) -> Result<()> {
        e.from_value.annotation(toks!["@", self.creator.clone()]);
        e.to_value.annotation(toks!["@", self.value.clone()]);
        Ok(())
    }
}

impl InterfaceCodegen for Jackson {
    fn generate(&self, InterfaceAdded { spec, body, .. }: InterfaceAdded) -> Result<()> {
        match body.sub_type_strategy {
            RpSubTypeStrategy::Tagged { ref tag, .. } => {
                let mut args = Tokens::new();
                args.append(toks!["use=", self.type_info.clone(), ".Id.NAME"]);
                args.append(toks!["include=", self.type_info.clone(), ".As.PROPERTY"]);
                args.append(toks!["property=", tag.as_str().quoted()]);
                spec.annotation(TypeInfo(self, args));
            }
        }

        {
            let mut args = Tokens::new();

            for sub_type in &body.sub_types {
                let mut a = Tokens::new();

                a.append(toks!["name=", sub_type.name().quoted()]);
                a.append(toks![
                    "value=",
                    spec.name(),
                    ".",
                    sub_type.ident.as_str(),
                    ".class",
                ]);

                let arg = SubTypesType(self, a).into_tokens();
                args.append(arg);
            }

            spec.annotation(SubTypes(self, args));
        }

        Ok(())
    }
}
