//! gRPC module for Rust.

use {Compiler, EnumAdded, EnumCodegen, FileSpec, InterfaceAdded, InterfaceCodegen,
     InterfaceModelAdded, InterfaceModelCodegen, Options, PackageAdded, PackageCodegen,
     StructModelAdded, StructModelCodegen, TupleAdded, TupleCodegen};
use backend::Initializer;
use core::{RpEnumBody, RpField, RpInterfaceBody, RpPackage, RpSubTypeStrategy, RpVersionedPackage};
use core::errors::{Error, Result};
use genco::{Quoted, Tokens};
use std::rc::Rc;
use swift::Swift;

pub struct Module {}

impl Module {
    pub fn new() -> Module {
        Module {}
    }
}

impl Initializer for Module {
    type Options = Options;

    fn initialize(&self, options: &mut Self::Options) -> Result<()> {
        let codegen = Rc::new(Codegen);
        options.struct_model_extends.append("Codable");
        options.tuple_gens.push(Box::new(codegen.clone()));
        options.struct_model_gens.push(Box::new(codegen.clone()));
        options.enum_gens.push(Box::new(codegen.clone()));
        options.interface_gens.push(Box::new(codegen.clone()));
        options.interface_model_gens.push(Box::new(codegen.clone()));
        options.any_type.push(("codable", "AnyCodable".into()));
        options.package_gens.push(Box::new(codegen.clone()));
        Ok(())
    }
}

struct Codegen;

impl Codegen {
    fn utils_package(&self) -> RpVersionedPackage {
        let package = RpPackage::new(vec!["ReprotoCodable_Utils".to_string()]);
        RpVersionedPackage::new(package, None)
    }

    fn utils<'el>(&self) -> Result<FileSpec<'el>> {
        let mut out = FileSpec::default();

        out.0.push(any_codable()?);

        return Ok(out);

        fn any_codable<'el>() -> Result<Tokens<'el, Swift<'el>>> {
            let primitives = vec![
                "Bool", "Int", "UInt", "Int32", "Int64", "UInt32", "UInt64", "Float", "Double",
                "String",
            ];

            let mut t = Tokens::new();

            t.try_push_into::<Error, _>(|t| {
                t.push("class AnyCodable: Codable {");

                t.nested({
                    let mut t = Tokens::new();

                    t.push("public let value: Any");
                    t.push(init()?);
                    t.push(encode()?);

                    t.push(decoding_error()?);
                    t.push(encoding_error()?);

                    t.push(decode_single(&primitives)?);
                    t.push(decode_unkeyed(&primitives)?);
                    t.push(decode_keyed(&primitives)?);
                    t.push(decode_array()?);
                    t.push(decode_dictionary()?);

                    t.push(encode_single(&primitives)?);
                    t.push(encode_unkeyed(&primitives)?);
                    t.push(encode_keyed(&primitives)?);

                    t.join_line_spacing()
                });

                t.push("}");
                Ok(())
            })?;

            t.push(any_coding_key());
            t.push(any_null());

            return Ok(t);

            fn init<'el>() -> Result<Tokens<'el, Swift<'el>>> {
                let mut t = Tokens::new();

                t.push("public required init(from decoder: Decoder) throws {");

                t.nested_into(|t| {
                    t.push_into(|t| {
                        t.push("if var array = try? decoder.unkeyedContainer() {");
                        t.nested("self.value = try AnyCodable.decodeArray(from: &array)");
                        t.nested("return");
                        t.push("}");
                    });

                    t.push_into(|t| {
                        t.push("if var c = try? decoder.container(keyedBy: AnyCodingKey.self) {");
                        t.nested("self.value = try AnyCodable.decodeDictionary(from: &c)");
                        t.nested("return");
                        t.push("}");
                    });

                    t.push_into(|t| {
                        t.push("let c = try decoder.singleValueContainer()");
                        t.push("self.value = try AnyCodable.decode(from: c)");
                    });
                });

                t.push("}");

                Ok(t)
            }

            fn encode<'el>() -> Result<Tokens<'el, Swift<'el>>> {
                let mut t = Tokens::new();

                t.push("public func encode(to encoder: Encoder) throws {");

                t.nested_into(|t| {
                    t.push_into(|t| {
                        t.push("if let arr = self.value as? [Any] {");
                        t.nested("var c = encoder.unkeyedContainer()");
                        t.nested("try AnyCodable.encode(to: &c, array: arr)");
                        t.nested("return");
                        t.push("}");
                    });

                    t.push_into(|t| {
                        t.push("if let dict = self.value as? [String: Any] {");
                        t.nested("var c = encoder.container(keyedBy: AnyCodingKey.self)");
                        t.nested("try AnyCodable.encode(to: &c, dictionary: dict)");
                        t.nested("return");
                        t.push("}");
                    });

                    t.push_into(|t| {
                        t.push("var c = encoder.singleValueContainer()");
                        t.push("try AnyCodable.encode(to: &c, value: self.value)");
                    });
                });

                t.push("}");

                Ok(t)
            }

            fn decoding_error<'el>() -> Result<Tokens<'el, Swift<'el>>> {
                let mut t = Tokens::new();

                t.push(
                    "static func decodingError(forCodingPath codingPath: [CodingKey]) -> \
                     DecodingError {",
                );

                t.nested({
                    let mut args = Tokens::new();
                    args.push("codingPath: codingPath");
                    args.push(toks![
                        "debugDescription: ",
                        "Cannot decode AnyCodable".quoted()
                    ]);

                    let mut t = Tokens::new();
                    t.push(toks![
                        "let context = DecodingError.Context(",
                        args.join(", "),
                        ")"
                    ]);
                    t.push("return DecodingError.typeMismatch(AnyCodable.self, context)");
                    t
                });

                t.push("}");

                Ok(t)
            }

            fn encoding_error<'el>() -> Result<Tokens<'el, Swift<'el>>> {
                let mut t = Tokens::new();

                t.push(
                    "static func encodingError(forValue value: Any, codingPath: [CodingKey]) -> \
                     EncodingError {",
                );

                t.nested({
                    let mut args = Tokens::new();
                    args.push("codingPath: codingPath");
                    args.push(toks![
                        "debugDescription: ",
                        "Cannot encode AnyCodable".quoted()
                    ]);

                    let mut t = Tokens::new();
                    t.push(toks![
                        "let context = EncodingError.Context(",
                        args.join(", "),
                        ")"
                    ]);
                    t.push("return EncodingError.invalidValue(value, context)");
                    t
                });

                t.push("}");

                Ok(t)
            }

            fn decode_single<'el>(primitives: &[&'el str]) -> Result<Tokens<'el, Swift<'el>>> {
                let mut t = Tokens::new();

                t.push("static func decode(from c: SingleValueDecodingContainer) throws -> Any {");

                t.nested({
                    let mut t = Tokens::new();

                    for p in primitives.iter().cloned() {
                        t.push_into(|t| {
                            t.push(toks!["if let value = try? c.decode(", p, ".self) {"]);
                            t.nested("return value");
                            t.push("}");
                        });
                    }

                    t.push_into(|t| {
                        t.push(toks!["if c.decodeNil() {"]);
                        t.nested("return AnyNull()");
                        t.push("}");
                    });

                    t.push("throw decodingError(forCodingPath: c.codingPath)");

                    t.join_line_spacing()
                });

                t.push("}");

                Ok(t)
            }

            fn decode_unkeyed<'el>(primitives: &[&'el str]) -> Result<Tokens<'el, Swift<'el>>> {
                let mut t = Tokens::new();

                t.push(
                    "static func decode(from c: inout UnkeyedDecodingContainer) throws -> Any {",
                );

                t.nested({
                    let mut t = Tokens::new();

                    for p in primitives.iter().cloned() {
                        t.push_into(|t| {
                            t.push(toks!["if let value = try? c.decode(", p, ".self) {"]);
                            t.nested("return value");
                            t.push("}");
                        });
                    }

                    t.push_into(|t| {
                        t.push(toks!["if let value = try? c.decodeNil() {"]);

                        t.nested_into(|t| {
                            t.push("if value {");
                            t.nested("return AnyNull()");
                            t.push("}");
                        });

                        t.push("}");
                    });

                    t.push_into(|t| {
                        t.push("if var c = try? c.nestedUnkeyedContainer() {");
                        t.nested("return try decodeArray(from: &c)");
                        t.push("}");
                    });

                    t.push_into(|t| {
                        t.push("if var c = try? c.nestedContainer(keyedBy: AnyCodingKey.self) {");
                        t.nested("return try decodeDictionary(from: &c)");
                        t.push("}");
                    });

                    t.push("throw decodingError(forCodingPath: c.codingPath)");
                    t.join_line_spacing()
                });

                t.push("}");

                Ok(t)
            }

            fn decode_keyed<'el>(primitives: &[&'el str]) -> Result<Tokens<'el, Swift<'el>>> {
                let mut t = Tokens::new();

                t.push(
                    "static func decode(from c: inout KeyedDecodingContainer<AnyCodingKey>, \
                     forKey key: AnyCodingKey) throws -> Any {",
                );

                t.nested({
                    let mut t = Tokens::new();

                    for p in primitives.iter().cloned() {
                        t.push_into(|t| {
                            t.push(toks![
                                "if let value = try? c.decode(",
                                p,
                                ".self, forKey: key) {"
                            ]);
                            t.nested("return value");
                            t.push("}");
                        });
                    }

                    t.push_into(|t| {
                        t.push(toks!["if let value = try? c.decodeNil(forKey: key) {"]);

                        t.nested_into(|t| {
                            t.push("if value {");
                            t.nested("return AnyNull()");
                            t.push("}");
                        });

                        t.push("}");
                    });

                    t.push_into(|t| {
                        t.push("if var c = try? c.nestedUnkeyedContainer(forKey: key) {");
                        t.nested("return try decodeArray(from: &c)");
                        t.push("}");
                    });

                    t.push_into(|t| {
                        t.push(
                            "if var c = try? c.nestedContainer(keyedBy: AnyCodingKey.self, \
                             forKey: key) {",
                        );
                        t.nested("return try decodeDictionary(from: &c)");
                        t.push("}");
                    });

                    t.push("throw decodingError(forCodingPath: c.codingPath)");
                    t.join_line_spacing()
                });

                t.push("}");

                Ok(t)
            }

            fn decode_array<'el>() -> Result<Tokens<'el, Swift<'el>>> {
                let mut t = Tokens::new();

                t.push(
                    "static func decodeArray(from c: inout UnkeyedDecodingContainer) throws -> \
                     [Any] {",
                );

                t.nested({
                    let mut t = Tokens::new();

                    t.push("var array: [Any] = []");
                    t.push_into(|t| {
                        t.push("while !c.isAtEnd {");
                        t.nested("array.append(try decode(from: &c))");
                        t.push("}");
                    });
                    t.push("return array");

                    t.join_line_spacing()
                });

                t.push("}");

                Ok(t)
            }

            fn decode_dictionary<'el>() -> Result<Tokens<'el, Swift<'el>>> {
                let mut t = Tokens::new();

                t.push(
                    "static func decodeDictionary(from c: inout \
                     KeyedDecodingContainer<AnyCodingKey>) throws -> [String: Any] {",
                );

                t.nested({
                    let mut t = Tokens::new();

                    t.push("var dict = [String: Any]()");
                    t.push_into(|t| {
                        t.push("for key in c.allKeys {");
                        t.nested("dict[key.stringValue] = try decode(from: &c, forKey: key)");
                        t.push("}");
                    });
                    t.push("return dict");

                    t.join_line_spacing()
                });

                t.push("}");

                Ok(t)
            }

            fn encode_single<'el>(primitives: &[&'el str]) -> Result<Tokens<'el, Swift<'el>>> {
                let mut t = Tokens::new();

                t.push(
                    "static func encode(to c: inout SingleValueEncodingContainer, value: Any) \
                     throws {",
                );

                t.nested_into(|t| {
                    t.push("switch value {");

                    for p in primitives.iter().cloned() {
                        t.push_into(|t| {
                            t.push(toks!["case let value as ", p, ":"]);
                            t.nested("try c.encode(value)");
                        });
                    }

                    t.push("case _ as AnyNull:");
                    t.nested("try c.encodeNil()");

                    t.push("default:");
                    t.nested("throw encodingError(forValue: value, codingPath: c.codingPath)");

                    t.push("}");
                });

                t.push("}");

                Ok(t)
            }

            fn encode_unkeyed<'el>(primitives: &[&'el str]) -> Result<Tokens<'el, Swift<'el>>> {
                let mut t = Tokens::new();

                t.push(
                    "static func encode(to c: inout UnkeyedEncodingContainer, array: [Any]) \
                     throws {",
                );

                t.nested_into(|t| {
                    t.push("for value in array {");
                    t.nested_into(|t| {
                        t.push("switch value {");

                        for p in primitives.iter().cloned() {
                            t.push_into(|t| {
                                t.push(toks!["case let value as ", p, ":"]);
                                t.nested("try c.encode(value)");
                            });
                        }

                        t.push("case let value as [Any]:");
                        t.nested("var c = c.nestedUnkeyedContainer()");
                        t.nested("try encode(to: &c, array: value)");

                        t.push("case let value as [String: Any]:");
                        t.nested("var c = c.nestedContainer(keyedBy: AnyCodingKey.self)");
                        t.nested("try encode(to: &c, dictionary: value)");

                        t.push("case _ as AnyNull:");
                        t.nested("try c.encodeNil()");

                        t.push("default:");
                        t.nested("throw encodingError(forValue: value, codingPath: c.codingPath)");

                        t.push("}");
                    });

                    t.push("}");
                });

                t.push("}");

                Ok(t)
            }

            fn encode_keyed<'el>(primitives: &[&'el str]) -> Result<Tokens<'el, Swift<'el>>> {
                let mut t = Tokens::new();

                t.push(
                    "static func encode(to c: inout KeyedEncodingContainer<AnyCodingKey>, \
                     dictionary: [String: Any]) throws {",
                );

                t.nested_into(|t| {
                    t.push("for (key, value) in dictionary {");
                    t.nested_into(|t| {
                        t.push("let key = AnyCodingKey(stringValue: key)!");

                        t.push_into(|t| {
                            t.push("switch value {");

                            for p in primitives.iter().cloned() {
                                t.push_into(|t| {
                                    t.push(toks!["case let value as ", p, ":"]);
                                    t.nested("try c.encode(value, forKey: key)");
                                });
                            }

                            t.push("case let value as [Any]:");
                            t.nested("var c = c.nestedUnkeyedContainer(forKey: key)");
                            t.nested("try encode(to: &c, array: value)");

                            t.push("case let value as [String: Any]:");
                            t.nested(
                                "var c = c.nestedContainer(keyedBy: AnyCodingKey.self, forKey: \
                                 key)",
                            );
                            t.nested("try encode(to: &c, dictionary: value)");

                            t.push("case _ as AnyNull:");
                            t.nested("try c.encodeNil(forKey: key)");

                            t.push("default:");
                            t.nested(
                                "throw encodingError(forValue: value, codingPath: c.codingPath)",
                            );

                            t.push("}");
                        });
                    });

                    t.push("}");
                });

                t.push("}");

                Ok(t)
            }

            fn any_coding_key<'el>() -> Tokens<'el, Swift<'el>> {
                let mut t = Tokens::new();

                t.push("class AnyCodingKey: CodingKey {");

                t.nested({
                    let mut t = Tokens::new();

                    t.push("let key: String");

                    t.push_into(|t| {
                        t.push("required init?(intValue: Int) {");
                        t.nested("return nil");
                        t.push("}");
                    });

                    t.push_into(|t| {
                        t.push("required init?(stringValue: String) {");
                        t.nested("key = stringValue");
                        t.push("}");
                    });

                    t.push_into(|t| {
                        t.push("var intValue: Int? {");
                        t.nested("return nil");
                        t.push("}");
                    });

                    t.push_into(|t| {
                        t.push("var stringValue: String {");
                        t.nested("return key");
                        t.push("}");
                    });

                    t.join_line_spacing()
                });

                t.push("}");

                t
            }

            fn any_null<'el>() -> Tokens<'el, Swift<'el>> {
                let mut t = Tokens::new();

                t.push("class AnyNull: Codable {");

                t.nested({
                    let mut t = Tokens::new();

                    t.push_into(|t| {
                        t.push("public init() {");
                        t.push("}");
                    });

                    t.push_into(|t| {
                        t.push("public required init(from decoder: Decoder) throws {");

                        let mut args = Tokens::new();
                        args.append("codingPath: decoder.codingPath");
                        args.append(toks![
                            "debugDescription: ",
                            "Wrong type for AnyNull".quoted()
                        ]);

                        let cx = toks!["DecodingError.Context(", args.join(", "), ")"];

                        t.nested_into(|t| {
                            t.push("let c = try decoder.singleValueContainer()");
                            t.push("if !c.decodeNil() {");

                            t.nested(toks![
                                "throw DecodingError.typeMismatch(AnyNull.self, ",
                                cx,
                                ")"
                            ]);

                            t.push("}");
                        });

                        t.push("}");
                    });

                    t.push_into(|t| {
                        t.push("public func encode(to encoder: Encoder) throws {");

                        t.nested_into(|t| {
                            t.push("var c = encoder.singleValueContainer()");
                            t.push("try c.encodeNil()");
                        });

                        t.push("}");
                    });

                    t.join_line_spacing()
                });

                t.push("}");

                t
            }
        }
    }
}

impl TupleCodegen for Codegen {
    fn generate(&self, e: TupleAdded) -> Result<()> {
        let TupleAdded {
            container,
            compiler,
            name,
            fields,
        } = e;

        container.push(decodable(compiler, name.clone(), fields)?);
        container.push(encodable(name.clone(), fields)?);

        return Ok(());

        fn decodable<'a>(
            compiler: &Compiler,
            name: Tokens<'a, Swift<'a>>,
            fields: &[&'a RpField],
        ) -> Result<Tokens<'a, Swift<'a>>> {
            let mut t = Tokens::new();
            t.push(toks!["extension ", name, ": Decodable {"]);
            t.nested(init(compiler, fields)?);
            t.push("}");

            return Ok(t);

            fn init<'a>(
                compiler: &Compiler,
                fields: &[&'a RpField],
            ) -> Result<Tokens<'a, Swift<'a>>> {
                let mut t = Tokens::new();

                t.push("public init(from decoder: Decoder) throws {");

                t.nested({
                    let mut t = Tokens::new();

                    t.push("var values = try decoder.unkeyedContainer()");

                    t.push({
                        let mut t = Tokens::new();

                        for field in fields {
                            let s = toks!["self.", field.safe_ident()];
                            let ty = compiler.field_type(&field.ty)?;

                            if field.is_optional() {
                                t.push(toks![s, " = try values.decodeIfPresent(", ty, ".self)"]);
                            } else {
                                t.push(toks![s, " = try values.decode(", ty, ".self)"]);
                            }
                        }

                        t
                    });

                    t.join_line_spacing()
                });

                t.push("}");

                Ok(t)
            }
        }

        fn encodable<'a>(
            name: Tokens<'a, Swift<'a>>,
            fields: &[&'a RpField],
        ) -> Result<Tokens<'a, Swift<'a>>> {
            let mut t = Tokens::new();
            t.push(toks!["extension ", name, ": Encodable {"]);

            t.push({
                let mut t = Tokens::new();
                t.nested(encode(fields)?);
                t.join_line_spacing()
            });

            t.push("}");

            return Ok(t);

            fn encode<'a>(fields: &[&'a RpField]) -> Result<Tokens<'a, Swift<'a>>> {
                let mut t = Tokens::new();

                t.push("public func encode(to encoder: Encoder) throws {");

                t.nested({
                    let mut t = Tokens::new();

                    t.push("var values = encoder.unkeyedContainer()");

                    t.push({
                        let mut t = Tokens::new();

                        for field in fields {
                            let s = toks!["self.", field.safe_ident()];

                            if field.is_optional() {
                                let var = field.safe_ident();

                                t.push({
                                    let mut t = Tokens::new();

                                    t.push(toks!["if let ", var.clone(), " = ", s, "{"]);
                                    t.nested(toks!["try values.encode(", var, ")"]);
                                    t.push("}");

                                    t
                                });
                            } else {
                                t.push(toks!["try values.encode(", s, ")"]);
                            }
                        }

                        t
                    });

                    t.join_line_spacing()
                });

                t.push("}");

                Ok(t)
            }
        }
    }
}

impl EnumCodegen for Codegen {
    fn generate(&self, e: EnumAdded) -> Result<()> {
        let EnumAdded {
            container,
            name,
            body,
            ..
        } = e;

        container.push(decodable(name.clone(), body)?);
        container.push(encodable(name.clone(), body)?);

        return Ok(());

        fn decodable<'a>(
            name: Tokens<'a, Swift<'a>>,
            body: &'a RpEnumBody,
        ) -> Result<Tokens<'a, Swift<'a>>> {
            let mut t = Tokens::new();
            t.push(toks!["extension ", name, ": Decodable {"]);
            t.nested(init(body)?);
            t.push("}");

            return Ok(t);

            fn init<'a>(body: &'a RpEnumBody) -> Result<Tokens<'a, Swift<'a>>> {
                let mut t = Tokens::new();

                t.push("public init(from decoder: Decoder) throws {");

                t.nested({
                    let mut t = Tokens::new();

                    t.push("let value = try decoder.singleValueContainer()");

                    t.push({
                        let mut t = Tokens::new();

                        t.push("switch try value.decode(String.self) {");

                        for variant in &body.variants {
                            t.push({
                                let mut t = Tokens::new();
                                t.push(toks!["case ", variant.ordinal().quoted(), ":"]);
                                t.nested(toks!["self = .", variant.local_name.as_str()]);
                                t
                            });
                        }

                        t.push({
                            let mut t = Tokens::new();

                            t.push("default:");
                            t.nested(toks![
                                "let context = DecodingError.Context(codingPath: [], \
                                 debugDescription: ",
                                "enum variant".quoted(),
                                ")"
                            ]);
                            t.nested("throw DecodingError.dataCorrupted(context)");

                            t
                        });
                        t.push("}");

                        t
                    });

                    t.join_line_spacing()
                });

                t.push("}");

                Ok(t)
            }
        }

        fn encodable<'a>(
            name: Tokens<'a, Swift<'a>>,
            body: &'a RpEnumBody,
        ) -> Result<Tokens<'a, Swift<'a>>> {
            let mut t = Tokens::new();
            t.push(toks!["extension ", name, ": Encodable {"]);

            t.push({
                let mut t = Tokens::new();
                t.nested(encode(body)?);
                t.join_line_spacing()
            });

            t.push("}");

            return Ok(t);

            fn encode<'a>(body: &'a RpEnumBody) -> Result<Tokens<'a, Swift<'a>>> {
                let mut t = Tokens::new();

                t.push("public func encode(to encoder: Encoder) throws {");

                t.nested({
                    let mut t = Tokens::new();

                    t.push("var value = encoder.singleValueContainer()");

                    t.push({
                        let mut t = Tokens::new();

                        t.push("switch self {");

                        for variant in &body.variants {
                            t.push({
                                let mut t = Tokens::new();
                                t.push(toks!["case .", variant.local_name.as_str(), ":"]);
                                t.nested(toks![
                                    "try value.encode(",
                                    variant.ordinal().quoted(),
                                    ")"
                                ]);
                                t
                            });
                        }

                        t.push("}");

                        t
                    });

                    t.join_line_spacing()
                });

                t.push("}");

                Ok(t)
            }
        }
    }
}

impl StructModelCodegen for Codegen {
    fn generate(&self, e: StructModelAdded) -> Result<()> {
        let StructModelAdded {
            container, fields, ..
        } = e;

        if fields.is_empty() {
            return Ok(());
        }

        container.push({
            let mut t = Tokens::new();

            t.push("enum CodingKeys: String, CodingKey {");

            for field in fields.iter() {
                t.nested(toks![
                    "case ",
                    field.safe_ident(),
                    " = ",
                    field.name().quoted()
                ]);
            }

            t.push("}");

            t
        });

        Ok(())
    }
}

impl InterfaceCodegen for Codegen {
    fn generate(&self, e: InterfaceAdded) -> Result<()> {
        let InterfaceAdded {
            container,
            compiler,
            name,
            body,
            ..
        } = e;

        container.push(decodable(compiler, name.clone(), body)?);
        container.push(encodable(name.clone(), body)?);

        return Ok(());

        fn decodable<'a>(
            compiler: &Compiler,
            name: Tokens<'a, Swift<'a>>,
            body: &'a RpInterfaceBody,
        ) -> Result<Tokens<'a, Swift<'a>>> {
            let mut t = Tokens::new();
            t.push(toks!["extension ", name, ": Decodable {"]);

            t.push({
                let mut t = Tokens::new();

                match body.sub_type_strategy {
                    RpSubTypeStrategy::Tagged { ref tag, .. } => {
                        t.nested(init(compiler, body, tag)?);
                    }
                }

                t.join_line_spacing()
            });

            t.push("}");

            return Ok(t);

            fn init<'a>(
                compiler: &Compiler,
                body: &'a RpInterfaceBody,
                tag: &'a str,
            ) -> Result<Tokens<'a, Swift<'a>>> {
                let mut t = Tokens::new();

                t.push("public init(from decoder: Decoder) throws {");

                t.nested({
                    let mut t = Tokens::new();

                    t.push("let values = try decoder.container(keyedBy: CodingKeys.self)");

                    t.push({
                        let mut t = Tokens::new();

                        t.push("switch try values.decode(String.self, forKey: .tag) {");

                        for sub_type in body.sub_types.values() {
                            t.push({
                                let mut t = Tokens::new();

                                let name = compiler.convert_name(&sub_type.name)?;
                                let n = sub_type.local_name.as_str();

                                let d = toks![name, "(from: decoder)"];
                                let d = toks![".", n, "(", d, ")"];

                                t.push(toks!["case ", sub_type.name().quoted(), ":"]);
                                t.nested(toks!["self = try ", d]);

                                t
                            });
                        }

                        t.push({
                            let mut t = Tokens::new();

                            t.push("default:");
                            t.nested(toks![
                                "let context = DecodingError.Context(codingPath: [], \
                                 debugDescription: ",
                                tag.quoted(),
                                ")"
                            ]);
                            t.nested("throw DecodingError.dataCorrupted(context)");

                            t
                        });
                        t.push("}");

                        t
                    });

                    t.join_line_spacing()
                });

                t.push("}");

                Ok(t)
            }
        }

        fn encodable<'a>(
            name: Tokens<'a, Swift<'a>>,
            body: &'a RpInterfaceBody,
        ) -> Result<Tokens<'a, Swift<'a>>> {
            let mut t = Tokens::new();
            t.push(toks!["extension ", name, ": Encodable {"]);

            t.push({
                let mut t = Tokens::new();
                t.nested(encode(body)?);
                t.join_line_spacing()
            });

            t.push("}");

            return Ok(t);

            fn encode<'a>(body: &'a RpInterfaceBody) -> Result<Tokens<'a, Swift<'a>>> {
                let mut t = Tokens::new();

                t.push("public func encode(to encoder: Encoder) throws {");

                t.nested({
                    let mut t = Tokens::new();

                    t.push("var values = encoder.container(keyedBy: CodingKeys.self)");

                    t.push({
                        let mut t = Tokens::new();

                        t.push("switch self {");

                        for sub_type in body.sub_types.values() {
                            let n = sub_type.local_name.as_str();
                            let name = sub_type.name();
                            let ty = toks!["try values.encode(", name.quoted(), ", forKey: .tag)"];

                            t.push({
                                let mut t = Tokens::new();
                                t.push(toks!["case .", n, "(let d):"]);
                                t.nested(ty);
                                t.nested(toks!["try d.encode(to: encoder)"]);
                                t
                            });
                        }

                        t.push("}");

                        t
                    });

                    t.join_line_spacing()
                });

                t.push("}");

                Ok(t)
            }
        }
    }
}

impl InterfaceModelCodegen for Codegen {
    fn generate(&self, e: InterfaceModelAdded) -> Result<()> {
        let InterfaceModelAdded {
            container, body, ..
        } = e;

        match body.sub_type_strategy {
            RpSubTypeStrategy::Tagged { ref tag, .. } => {
                container.nested({
                    let mut t = Tokens::new();

                    t.push({
                        let mut t = Tokens::new();

                        t.push("enum CodingKeys: String, CodingKey {");
                        t.nested(toks!["case tag = ", tag.as_str().quoted()]);
                        t.push("}");

                        t
                    });

                    t
                });
            }
        }

        Ok(())
    }
}

impl PackageCodegen for Codegen {
    fn generate(&self, e: PackageAdded) -> Result<()> {
        let PackageAdded { files, .. } = e;
        files.insert(self.utils_package(), self.utils()?);
        Ok(())
    }
}
