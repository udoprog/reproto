/// Module that adds fasterxml annotations to generated classes.
use backend::*;
use codeviz::java::*;
use super::models as m;
use super::processor::*;

pub struct Module {
    override_: ClassType,
    creator: ClassType,
    value: ClassType,
    property: ClassType,
    sub_types: ClassType,
    type_info: ClassType,
    serialize: ClassType,
    deserialize: ClassType,
    deserializer: ClassType,
    serializer: ClassType,
    generator: ClassType,
    serializer_provider: ClassType,
    parser: ClassType,
    deserialization_context: ClassType,
    token: ClassType,
    string: ClassType,
    io_exception: ClassType,
}

impl Module {
    pub fn new() -> Module {
        Module {
            override_: Type::class("java.lang", "Override"),
            creator: Type::class("com.fasterxml.jackson.annotation", "JsonCreator"),
            value: Type::class("com.fasterxml.jackson.annotation", "JsonValue"),
            property: Type::class("com.fasterxml.jackson.annotation", "JsonProperty"),
            sub_types: Type::class("com.fasterxml.jackson.annotation", "JsonSubTypes"),
            type_info: Type::class("com.fasterxml.jackson.annotation", "JsonTypeInfo"),
            serialize: Type::class("com.fasterxml.jackson.databind.annotation", "JsonSerialize"),
            deserialize: Type::class("com.fasterxml.jackson.databind.annotation",
                                     "JsonDeserialize"),
            serializer: Type::class("com.fasterxml.jackson.databind", "JsonSerializer"),
            deserializer: Type::class("com.fasterxml.jackson.databind", "JsonDeserializer"),
            generator: Type::class("com.fasterxml.jackson.core", "JsonGenerator"),
            serializer_provider: Type::class("com.fasterxml.jackson.databind",
                                             "SerializerProvider"),
            parser: Type::class("com.fasterxml.jackson.core", "JsonParser"),
            deserialization_context: Type::class("com.fasterxml.jackson.databind",
                                                 "DeserializationContext"),
            token: Type::class("com.fasterxml.jackson.core", "JsonToken"),
            string: Type::class("java.lang", "String"),
            io_exception: Type::class("java.io", "IOException"),
        }
    }

    /// Custom serialize implementation for tuples.
    fn tuple_serializer(&self,
                        fields: &Vec<m::JavaField>,
                        class_type: &ClassType)
                        -> Result<ClassSpec> {
        let mut serializer = ClassSpec::new(mods![Modifier::Public, Modifier::Static],
                                            "Serializer");

        serializer.extends(self.serializer.with_arguments(vec![&class_type]));

        let value = ArgumentSpec::new(mods![Modifier::Final], &class_type, "value");
        let jgen = ArgumentSpec::new(mods![Modifier::Final], &self.generator, "jgen");
        let provider = ArgumentSpec::new(mods![Modifier::Final],
                                         &self.serializer_provider,
                                         "provider");

        let mut serialize = MethodSpec::new(mods![Modifier::Public], "serialize");
        serialize.throws(&self.io_exception);
        serialize.push_argument(&value);
        serialize.push_argument(&jgen);
        serialize.push_argument(&provider);
        serialize.push_annotation(&self.override_);

        let mut body = Elements::new();
        body.push(stmt![&jgen, ".writeStartArray();"]);

        for field in fields {
            let field_stmt = stmt![&value, ".", &field.spec];

            let write = match field.ty {
                Type::Primitive(ref primitive) => {
                    match *primitive {
                        SHORT | LONG | INTEGER | FLOAT | DOUBLE => {
                            stmt!["writeNumber(", field_stmt, ")"]
                        }
                        _ => return Err("cannot serialize type".into()),
                    }
                }
                Type::Class(ref class) => {
                    if *class == self.string {
                        stmt!["writeString(", field_stmt, ")"]
                    } else {
                        stmt!["writeObject(", field_stmt, ")"]
                    }
                }
                _ => stmt!["writeObject(", field_stmt, ")"],
            };

            body.push(stmt![&jgen, ".", write, ";"]);
        }

        body.push(stmt![&jgen, ".writeEndArray();"]);

        serialize.push(body);

        serializer.push(serialize);

        Ok(serializer)
    }

    fn deserialize_method_for_type(&self,
                                   ty: &Type,
                                   parser: &ArgumentSpec)
                                   -> Result<(Option<(Statement, &str)>, Statement)> {
        match *ty {
            Type::Primitive(ref primitive) => {
                let test = stmt!["!", parser, ".nextToken().isNumeric()"];

                match *primitive {
                    SHORT => {
                        Ok((Some((test, "VALUE_NUMBER_INT")), stmt![parser, ".getShortValue()"]))
                    }
                    LONG => {
                        Ok((Some((test, "VALUE_NUMBER_INT")), stmt![parser, ".getLongValue()"]))
                    }
                    INTEGER => {
                        Ok((Some((test, "VALUE_NUMBER_INT")), stmt![parser, ".getIntegerValue()"]))
                    }
                    FLOAT => {
                        Ok((Some((test, "VALUE_NUMBER_FLOAT")), stmt![parser, ".getFloatValue()"]))
                    }
                    DOUBLE => {
                        Ok((Some((test, "VALUE_NUMBER_FLOAT")), stmt![parser, ".getDoubleValue()"]))
                    }
                    _ => return Err("cannot deserialize type".into()),
                }
            }
            Type::Class(ref class) => {
                if *class == self.string {
                    let test = stmt![&parser, ".nextToken() != ", &self.token, ".VALUE_STRING"];
                    let token = Some((test, "VALUE_STRING"));
                    return Ok((token, stmt![parser, ".getText()"]));
                }

                if class.arguments.is_empty() {
                    return Ok((None, stmt![parser, ".readValueAs(", class, ".class)"]));
                }

                // TODO: support generics
                return Err("cannot deserialize type".into());
            }
            Type::Local(ref local) => {
                return Ok((None, stmt![parser, ".readValueAs(", &local.name, ")"]));
            }
        }
    }

    fn wrong_token_exception(&self,
                             ctxt: &ArgumentSpec,
                             parser: &ArgumentSpec,
                             token: &str)
                             -> Statement {
        let mut arguments = Statement::new();
        arguments.push(parser);
        arguments.push(stmt![&self.token, ".", token]);
        arguments.push("null");

        stmt!["throw ", ctxt, ".wrongTokenException(", arguments.join(", "), ");"]
    }

    /// Custom deserialize implementation for tuples.
    fn tuple_deserializer(&self,
                          fields: &Vec<m::JavaField>,
                          class_type: &ClassType)
                          -> Result<ClassSpec> {
        let mut deserializer = ClassSpec::new(mods![Modifier::Public, Modifier::Static],
                                              "Deserializer");

        deserializer.extends(self.deserializer.with_arguments(vec![&class_type]));

        let parser = ArgumentSpec::new(mods![Modifier::Final], &self.parser, "parser");
        let ctxt = ArgumentSpec::new(mods![Modifier::Final],
                                     &self.deserialization_context,
                                     "ctxt");

        let mut deserialize = MethodSpec::new(mods![Modifier::Public], "deserialize");
        deserialize.throws(&self.io_exception);
        deserialize.push_argument(&parser);
        deserialize.push_argument(&ctxt);
        deserialize.push_annotation(&self.override_);
        deserialize.returns(&class_type);

        let current_token = stmt![&parser, ".getCurrentToken()"];

        let mut start_array = Elements::new();
        start_array.push(stmt!["if (", &current_token, " != ", &self.token, ".START_ARRAY) {"]);
        start_array.push_nested(self.wrong_token_exception(&ctxt, &parser, "START_ARRAY"));
        start_array.push("}");
        deserialize.push(start_array);

        let mut arguments = Statement::new();

        for field in fields {
            let (token, reader) = self.deserialize_method_for_type(&field.ty, &parser)?;

            if let Some((test, expected)) = token {
                let mut field_check = Elements::new();
                field_check.push(stmt!["if (", &test, ") {"]);
                field_check.push_nested(self.wrong_token_exception(&ctxt, &parser, expected));
                field_check.push("}");
                deserialize.push(field_check);
            }

            let variable = stmt!["v_", &field.spec.name];
            let assign = stmt!["final ", &field.spec.ty, " ", &variable, " = ", reader, ";"];
            deserialize.push(assign);
            arguments.push(variable);
        }

        let next_token = stmt![&parser, ".nextToken()"];

        let mut end_array = Elements::new();
        end_array.push(stmt!["if (", &next_token, " != ", &self.token, ".END_ARRAY) {"]);
        end_array.push_nested(self.wrong_token_exception(&ctxt, &parser, "END_ARRAY"));
        end_array.push("}");
        deserialize.push(end_array);

        deserialize.push(stmt!["return new ", &class_type, "(", arguments.join(", "), ");"]);

        deserializer.push(deserialize);
        Ok(deserializer)
    }
}

impl Listeners for Module {
    fn class_added(&self, event: &mut ClassAdded) -> Result<()> {
        if event.spec.constructors.len() != 1 {
            return Err("Expected exactly one constructor".into());
        }

        let constructor = &mut event.spec.constructors[0];

        if constructor.arguments.len() != event.fields.len() {
            return Err(format!("{:?}: the number of constructor arguments ({}) did not match \
                                the number of fields ({})",
                               event.class_type,
                               constructor.arguments.len(),
                               event.fields.len())
                .into());
        }

        let creator_annotation = AnnotationSpec::new(&self.creator);
        constructor.push_annotation(&creator_annotation);

        let zipped = constructor.arguments.iter_mut().zip(event.fields.iter());

        for (argument, field) in zipped {
            let mut property = AnnotationSpec::new(&self.property);
            property.push_argument(stmt![Variable::String(field.name.clone())]);
            argument.push_annotation(&property);
        }

        Ok(())
    }

    fn tuple_added(&self, event: &mut TupleAdded) -> Result<()> {
        let serializer = self.tuple_serializer(&event.fields, &event.class_type)?;

        let serializer_type =
            Type::class(&event.class_type.package,
                        &format!("{}.{}", event.class_type.name, serializer.name));

        let mut serialize_annotation: AnnotationSpec = self.serialize.clone().into();
        serialize_annotation.push_argument(stmt!["using = ", serializer_type, ".class"]);

        event.spec.push_annotation(serialize_annotation);
        event.spec.push(serializer);

        let deserializer = self.tuple_deserializer(&event.fields, &event.class_type)?;

        let deserializer_type =
            Type::class(&event.class_type.package,
                        &format!("{}.{}", &event.class_type.name, deserializer.name));

        let mut deserialize_annotation: AnnotationSpec = self.deserialize.clone().into();
        deserialize_annotation.push_argument(stmt!["using = ", deserializer_type, ".class"]);

        event.spec.push_annotation(deserialize_annotation);
        event.spec.push(deserializer);

        Ok(())
    }

    fn enum_added(&self, event: &mut EnumAdded) -> Result<()> {
        if let Some(ref mut from_value) = *event.from_value {
            from_value.push_annotation(&self.creator);
        }

        if let Some(ref mut to_value) = *event.to_value {
            to_value.push_annotation(&self.value);
        }

        Ok(())
    }

    fn interface_added(&self, event: &mut InterfaceAdded) -> Result<()> {
        {
            let mut arguments = Statement::new();

            arguments.push(stmt!["use=", &self.type_info, ".Id.NAME"]);
            arguments.push(stmt!["include=", &self.type_info, ".As.PROPERTY"]);
            arguments.push(stmt!["property=", Variable::String("type".to_owned())]);

            let mut type_info = AnnotationSpec::new(&self.type_info);
            type_info.push_argument(arguments.join(", "));

            event.spec.push_annotation(&type_info);
        }

        {
            let mut arguments = Statement::new();

            for (key, sub_type) in &event.interface.sub_types {
                for name in &sub_type.names {
                    let name: String = name.inner.to_owned();

                    let mut type_args = Statement::new();

                    type_args.push(stmt!["name=", Variable::String(name)]);
                    type_args.push(stmt!["value=", &event.spec.name, ".", key, ".class"]);

                    let a = stmt!["@", &self.sub_types, ".Type(", type_args.join(", "), ")"];

                    arguments.push(a);
                }
            }

            let mut sub_types = AnnotationSpec::new(&self.sub_types);
            sub_types.push_argument(stmt!["{", arguments.join(", "), "}"]);

            event.spec.push_annotation(&sub_types);
        }

        Ok(())
    }
}
