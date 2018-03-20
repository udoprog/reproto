use codegen::{ClassAdded, ClassCodegen, Configure, EnumAdded, EnumCodegen, InterfaceAdded,
              InterfaceCodegen, TupleAdded, TupleCodegen, TypeField, TypeFieldAdded,
              TypeFieldCodegen};
use core::RpSubTypeStrategy;
use core::errors::Result;
use genco::{Cons, Csharp, Element, IntoTokens, Quoted, Tokens};
use genco::csharp::{Argument, using};
use std::rc::Rc;

pub struct Module;

impl Module {
    pub fn initialize(self, e: Configure) {
        let json_net = Rc::new(JsonNet::new());

        e.options.class_generators.push(
            Box::new(Rc::clone(&json_net)),
        );

        e.options.enum_generators.push(
            Box::new(Rc::clone(&json_net)),
        );

        e.options.interface_generators.push(
            Box::new(Rc::clone(&json_net)),
        );

        e.options.type_field_generators.push(Box::new(
            Rc::clone(&json_net),
        ));

        e.options.tuple_generators.push(
            Box::new(Rc::clone(&json_net)),
        );
    }
}

/// Apply attributes.
struct JsonNet {
    object: Csharp<'static>,
    type_: Csharp<'static>,
    invalid_operation: Csharp<'static>,
    enumerator: Csharp<'static>,
    j_array: Csharp<'static>,
    j_token: Csharp<'static>,
    json_reader: Csharp<'static>,
    json_writer: Csharp<'static>,
    json_serializer: Csharp<'static>,
}

impl JsonNet {
    pub fn new() -> Self {
        Self {
            object: using("System", "Object"),
            type_: using("System", "Type").qualified(),
            invalid_operation: using("System", "InvalidOperationException"),
            enumerator: using("System.Collections.Generic", "IEnumerator"),
            j_array: using("Newtonsoft.Json.Linq", "JArray"),
            j_token: using("Newtonsoft.Json.Linq", "JToken"),
            json_reader: using("Newtonsoft.Json", "JsonReader"),
            json_writer: using("Newtonsoft.Json", "JsonWriter"),
            json_serializer: using("Newtonsoft.Json", "JsonSerializer"),
        }
    }
}

impl ClassCodegen for JsonNet {
    fn generate(&self, e: ClassAdded) -> Result<()> {
        let mut type_field = e.type_field;
        let names = &e.names;
        let spec = e.spec;
        let fields = e.fields;

        spec.attribute(JsonObject);

        // Annotate all constructors.
        for c in &mut spec.constructors {
            c.attribute(JsonConstructor);

            for (field, (argument, name)) in
                fields.iter().zip(c.arguments.iter_mut().zip(names.iter()))
            {
                let required = if field.optional {
                    Required::Default
                } else {
                    Required::DisallowNull
                };

                argument.attribute(JsonProperty(name.clone(), required));
            }

            // Modify the class to deserialize, and pass type field into the super class.
            if let Some(&mut TypeField {
                            ref mut field,
                            ref tag,
                        }) = type_field.as_mut()
            {
                let mut a = Argument::new(field.ty(), field.var());
                a.attribute(JsonProperty(tag.clone(), Required::DisallowNull));
                c.arguments.insert(0, a);
                c.base = Some(toks!["base(", field.var(), ")"]);
            }
        }

        // Add field attribute.
        for (field, (spec, name)) in fields.iter().zip(spec.fields.iter_mut().zip(names.iter())) {
            let required = if field.optional {
                Required::Default
            } else {
                Required::DisallowNull
            };

            spec.attribute(JsonProperty(name.clone(), required));
        }

        Ok(())
    }
}

impl EnumCodegen for JsonNet {
    fn generate(&self, e: EnumAdded) -> Result<()> {
        let spec = e.spec;
        let names = e.names;

        spec.attribute(StringEnumConverter);

        let mut variants = Tokens::new();

        for (v, name) in spec.variants.clone().into_iter().zip(names.iter().cloned()) {
            let mut annotated = Tokens::new();
            annotated.push(EnumMember(name));
            annotated.push(v);
            variants.push(annotated);
        }

        spec.variants = variants;
        Ok(())
    }
}

impl InterfaceCodegen for JsonNet {
    fn generate(&self, InterfaceAdded { spec, body, .. }: InterfaceAdded) -> Result<()> {
        match body.sub_type_strategy {
            RpSubTypeStrategy::Tagged { ref tag, .. } => {
                let tag = Rc::new(tag.to_string()).into();
                spec.attribute(JsonSubTypes(tag));
            }
        }

        for sub_type in &body.sub_types {
            let v = toks![spec.name(), ".", sub_type.ident.as_str()];
            spec.attribute(JsonSubType(sub_type.name().into(), v));
        }

        return Ok(());

        struct JsonSubTypes<'el>(Cons<'el>);

        impl<'el> IntoTokens<'el, Csharp<'el>> for JsonSubTypes<'el> {
            fn into_tokens(self) -> Tokens<'el, Csharp<'el>> {
                let converter = using("Newtonsoft.Json", "JsonConverter");
                let sub_types = using("JsonSubTypes", "JsonSubtypes");

                let mut args = Tokens::new();
                args.append(toks!["typeof(", sub_types, ")"]);
                args.append(Element::from(self.0.quoted()));

                toks!["[", converter, "(", args.join(", "), ")]"]
            }
        }

        struct JsonSubType<'el>(Cons<'el>, Tokens<'el, Csharp<'el>>);

        impl<'el> IntoTokens<'el, Csharp<'el>> for JsonSubType<'el> {
            fn into_tokens(self) -> Tokens<'el, Csharp<'el>> {
                let sub_types = using("JsonSubTypes", "JsonSubtypes");

                let mut args = Tokens::new();
                args.append(toks!["typeof(", self.1, ")"]);
                args.append(Element::from(self.0.quoted()));

                toks!["[", sub_types, ".KnownSubType(", args.join(", "), ")]"]
            }
        }
    }
}

impl TypeFieldCodegen for JsonNet {
    fn generate(&self, TypeFieldAdded { field, tag }: TypeFieldAdded) -> Result<()> {
        field.attribute(JsonProperty(tag.clone(), Required::DisallowNull));
        Ok(())
    }
}

impl TupleCodegen for JsonNet {
    fn generate(&self, TupleAdded { mut spec }: TupleAdded) -> Result<()> {
        use genco::csharp::{BOOLEAN, Class, Method, Modifier, local};

        let converter = Rc::new(format!("{}.Json_Net_Converter", spec.name().as_ref()));
        spec.attribute(JsonConverter(local(converter)));

        let body = {
            let mut c = Class::new("Json_Net_Converter");
            let converter = using("Newtonsoft.Json", "JsonConverter");
            c.implements = vec![converter];

            c.body.push(CanConvert(self, &spec));
            c.body.push(WriteJson(self, &mut spec));
            c.body.push(ReadJson(self, &spec));

            c
        };

        // Converter for this tuple.
        spec.body.push(body);

        return Ok(());

        #[allow(unused)]
        // public override bool CanConvert impl
        struct CanConvert<'a, 'el: 'a>(&'a JsonNet, &'a Class<'el>);

        impl<'a, 'el> IntoTokens<'el, Csharp<'el>> for CanConvert<'a, 'el> {
            fn into_tokens(self) -> Tokens<'el, Csharp<'el>> {
                let cls = local(self.1.name());

                let mut can_convert = Method::new("CanConvert");
                can_convert.arguments.push(Argument::new(
                    self.0.type_.clone(),
                    "objectType",
                ));
                can_convert.modifiers = vec![Modifier::Public, Modifier::Override];
                can_convert.returns = BOOLEAN;

                can_convert.body.push({
                    let ck = toks!["objectType == typeof(", cls.clone(), ")"];
                    toks!["return ", ck, ";"]
                });

                can_convert.into_tokens()
            }
        }

        // public override bool WriteJson impl
        struct WriteJson<'a, 'el: 'a>(&'a JsonNet, &'a Class<'el>);

        impl<'a, 'el> IntoTokens<'el, Csharp<'el>> for WriteJson<'a, 'el> {
            fn into_tokens(self) -> Tokens<'el, Csharp<'el>> {
                let mut read_json = Method::new("WriteJson");
                read_json.arguments.push(Argument::new(
                    self.0.json_writer.clone(),
                    "writer",
                ));
                read_json.arguments.push(Argument::new(
                    self.0.object.clone(),
                    "obj",
                ));
                read_json.arguments.push(Argument::new(
                    self.0.json_serializer.clone(),
                    "serializer",
                ));
                read_json.modifiers = vec![Modifier::Public, Modifier::Override];

                read_json.body.push({
                    let mut t = Tokens::new();

                    let ty = local(self.1.name());

                    t.push(toks![ty.clone(), " o = (", ty.clone(), ")obj;"]);

                    t.push(toks![
                        self.0.j_array.clone(),
                        " array = new ",
                        self.0.j_array.clone(),
                        "();",
                    ]);

                    for f in &self.1.fields {
                        let mut ser =
                            toks![
                            self.0.j_token.clone(),
                            ".FromObject(o.",
                            f.var(),
                            ", serializer)",
                        ];

                        t.push(toks!["array.Add(", ser, ");"]);
                    }

                    t.push("array.WriteTo(writer);");

                    t
                });

                read_json.into_tokens()
            }
        }

        // public override bool ReadJson impl
        struct ReadJson<'a, 'el: 'a>(&'a JsonNet, &'a Class<'el>);

        impl<'a, 'el> IntoTokens<'el, Csharp<'el>> for ReadJson<'a, 'el> {
            fn into_tokens(self) -> Tokens<'el, Csharp<'el>> {
                let cls = local(self.1.name());

                let mut read_json = Method::new("ReadJson");
                read_json.arguments.push(Argument::new(
                    self.0.json_reader.clone(),
                    "reader",
                ));
                read_json.arguments.push(Argument::new(
                    self.0.type_.clone(),
                    "objectType",
                ));
                read_json.arguments.push(Argument::new(
                    self.0.object.clone(),
                    "existingValue",
                ));
                read_json.arguments.push(Argument::new(
                    self.0.json_serializer.clone(),
                    "serializer",
                ));
                read_json.modifiers = vec![Modifier::Public, Modifier::Override];
                read_json.returns = self.0.object.clone();

                read_json.body.push(toks![
                    self.0.j_array.clone(),
                    " array = ",
                    self.0.j_array.clone(),
                    ".Load(reader);",
                ]);

                read_json.body.push(toks![
                    self.0.enumerator.with_arguments(
                        vec![self.0.j_token.clone()]
                    ),
                    " enumerator = array.GetEnumerator();",
                ]);

                let mut args = Tokens::new();

                for f in &self.1.fields {
                    read_json.body.push("if (!enumerator.MoveNext()) {");
                    let msg = "expected more items in array";
                    read_json.body.nested(toks![
                        "throw new ",
                        self.0.invalid_operation.clone(),
                        "(",
                        msg.quoted(),
                        ");",
                    ]);
                    read_json.body.push("}");

                    let to_object = toks!["enumerator.Current.ToObject<", f.ty(), ">(serializer);"];

                    read_json.body.push(toks![
                        f.ty(),
                        " ",
                        f.var(),
                        " = ",
                        to_object,
                        ";",
                    ]);

                    args.append(f.var());
                }

                read_json.body.push(toks![
                    "return new ",
                    cls.clone(),
                    "(",
                    args.join(", "),
                    ");",
                ]);

                read_json.into_tokens()
            }
        }
    }
}

#[allow(unused)]
#[derive(Clone, Copy, Debug)]
enum Required {
    Default,
    Always,
    AllowNull,
    DisallowNull,
}

impl<'el> IntoTokens<'el, Csharp<'el>> for Required {
    fn into_tokens(self) -> Tokens<'el, Csharp<'el>> {
        let required = using("Newtonsoft.Json", "Required");

        let out = match self {
            Required::Default => "Default",
            Required::Always => "Always",
            Required::AllowNull => "AllowNull",
            Required::DisallowNull => "DisallowNull",
        };

        toks![required, ".", out]
    }
}

#[allow(unused)]
#[derive(Clone, Copy, Debug)]
enum NullValueHandling {
    Include,
    Ignore,
}

impl<'el> IntoTokens<'el, Csharp<'el>> for NullValueHandling {
    fn into_tokens(self) -> Tokens<'el, Csharp<'el>> {
        let null_value_handling = using("Newtonsoft.Json", "NullValueHandling");

        let out = match self {
            NullValueHandling::Include => "Include",
            NullValueHandling::Ignore => "Ignore",
        };

        toks![null_value_handling, ".", out]
    }
}

/// [JsonObject(..)] attribute
pub struct JsonObject;

impl<'el> IntoTokens<'el, Csharp<'el>> for JsonObject {
    fn into_tokens(self) -> Tokens<'el, Csharp<'el>> {
        let object = using("Newtonsoft.Json", "JsonObject");

        let mut args: Tokens<'el, Csharp<'el>> = Tokens::new();

        args.append(toks![
            "ItemNullValueHandling = ",
            NullValueHandling::Ignore.into_tokens(),
        ]);

        toks!["[", object, "(", args.join(", "), ")]"]
    }
}

/// [JsonProperty(..)] attribute
pub struct JsonProperty<'el>(Cons<'el>, Required);

impl<'el> IntoTokens<'el, Csharp<'el>> for JsonProperty<'el> {
    fn into_tokens(self) -> Tokens<'el, Csharp<'el>> {
        let property = using("Newtonsoft.Json", "JsonProperty");

        let mut args: Tokens<'el, Csharp<'el>> = Tokens::new();

        args.append(self.0.quoted());

        match self.1 {
            Required::Default => {}
            other => args.append(toks!["Required = ", other.into_tokens()]),
        }

        toks!["[", property, "(", args.join(", "), ")]"]
    }
}

/// [JsonConstructor] attribute
pub struct JsonConstructor;

impl<'el> IntoTokens<'el, Csharp<'el>> for JsonConstructor {
    fn into_tokens(self) -> Tokens<'el, Csharp<'el>> {
        let constructor = using("Newtonsoft.Json", "JsonConstructor");
        toks!["[", constructor, "]"]
    }
}

/// [EnumMember(..)] attribute
pub struct EnumMember<'el>(Cons<'el>);

impl<'el> IntoTokens<'el, Csharp<'el>> for EnumMember<'el> {
    fn into_tokens(self) -> Tokens<'el, Csharp<'el>> {
        let enum_member = using("System.Runtime.Serialization", "EnumMember");

        let mut args: Tokens<'el, Csharp<'el>> = Tokens::new();
        args.append(toks!["Value = ", self.0.quoted()]);
        toks!["[", enum_member, "(", args.join(", "), ")]"]
    }
}

/// [EnumAttribute(..)] attribute
pub struct StringEnumConverter;

impl<'el> IntoTokens<'el, Csharp<'el>> for StringEnumConverter {
    fn into_tokens(self) -> Tokens<'el, Csharp<'el>> {
        JsonConverter(using("Newtonsoft.Json.Converters", "StringEnumConverter")).into_tokens()
    }
}

/// [JsonCoverter(..)] attribute
pub struct JsonConverter<'el>(Csharp<'el>);

impl<'el> IntoTokens<'el, Csharp<'el>> for JsonConverter<'el> {
    fn into_tokens(self) -> Tokens<'el, Csharp<'el>> {
        let converter = using("Newtonsoft.Json", "JsonConverter");

        toks!["[", converter, "(typeof(", self.0, "))]"]
    }
}
