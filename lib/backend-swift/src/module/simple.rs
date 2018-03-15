//! gRPC module for Rust.

use {Compiler, EnumAdded, EnumCodegen, FileSpec, InterfaceAdded, InterfaceCodegen, Options,
     PackageAdded, PackageCodegen, TupleAdded, TupleCodegen, TypeAdded, TypeCodegen};
use backend::Initializer;
use compiler::Comments;
use core::{Loc, RpEnumBody, RpField, RpPackage, RpSubType, RpSubTypeStrategy, RpType,
           RpVersionedPackage};
use core::errors::Result;
use genco::{Cons, IntoTokens, Quoted, Tokens};
use std::rc::Rc;
use swift::{imported, Swift};

pub struct GuardMissing<'el>(Cons<'el>, Tokens<'el, Swift<'el>>, Cons<'el>);

impl<'el> IntoTokens<'el, Swift<'el>> for GuardMissing<'el> {
    fn into_tokens(self) -> Tokens<'el, Swift<'el>> {
        let GuardMissing(dest, source, name) = self;

        let mut t = Tokens::new();

        t.push(toks!["guard let ", dest, " = ", source, " else {"]);

        t.nested(toks![
            "throw SerializationError.missing(",
            name.quoted(),
            ")"
        ]);

        t.push("}");

        t
    }
}

pub struct Module {}

impl Module {
    pub fn new() -> Module {
        Module {}
    }
}

impl Initializer for Module {
    type Options = Options;

    fn initialize(&self, options: &mut Self::Options) -> Result<()> {
        let codegen = Rc::new(Codegen::new());
        options.type_gens.push(Box::new(codegen.clone()));
        options.tuple_gens.push(Box::new(codegen.clone()));
        options.enum_gens.push(Box::new(codegen.clone()));
        options.interface_gens.push(Box::new(codegen.clone()));
        options.package_gens.push(Box::new(codegen.clone()));
        Ok(())
    }
}

struct Codegen {
    data: Swift<'static>,
    formatter: Swift<'static>,
}

impl Codegen {
    pub fn new() -> Codegen {
        Self {
            data: imported("Foundation", "Data"),
            formatter: imported("Foundation", "ISO8601DateFormatter"),
        }
    }

    /// Decode the given value.
    fn decode_value<'a>(
        &self,
        compiler: &Compiler,
        ty: &'a RpType,
        name: Cons<'a>,
        var: Tokens<'a, Swift<'a>>,
    ) -> Result<Tokens<'a, Swift<'a>>> {
        use self::RpType::*;

        let unbox = match *ty {
            String => unbox(var, "String"),
            DateTime => {
                let string = toks!["try decode_value(", var, " as? String)"];
                let date = toks![self.formatter.clone(), "().date(from: ", string, ")"];
                toks!["try decode_value(", date, ")"]
            }
            Bytes => toks![
                self.data.clone(),
                "(base64Encoded: try decode_value(",
                var,
                " as? String))"
            ],
            Signed { size: 32 } => unbox(var, "Int32"),
            Signed { size: 64 } => unbox(var, "Int64"),
            Unsigned { size: 32 } => unbox(var, "UInt32"),
            Unsigned { size: 64 } => unbox(var, "UInt64"),
            Float => unbox(var, "Float"),
            Double => unbox(var, "Double"),
            Boolean => unbox(var, "Bool"),
            Array { ref inner } => {
                let inner = self.decode_value(compiler, inner, name.clone(), "inner".into())?;
                return Ok(toks![
                    "try decode_array(",
                    var,
                    ", name: ",
                    name.quoted(),
                    ", inner: { inner in ",
                    inner,
                    " })"
                ]);
            }
            Map { ref value, .. } => {
                let value = self.decode_value(compiler, value, name.clone(), "value".into())?;
                return Ok(toks![
                    "try decode_map(",
                    var,
                    ", name: ",
                    name.quoted(),
                    ", value: { value in ",
                    value,
                    " })"
                ]);
            }
            Name {
                name: ref inner_name,
            } => {
                let inner_name = compiler.convert_name(inner_name)?;
                return Ok(toks!["try ", inner_name, ".decode(json: ", var, ")"]);
            }
            Any => toks![var],
            _ => return Err(format!("unsupported type: {}", ty).into()),
        };

        return Ok(toks![
            "try decode_name(",
            unbox,
            ", name: ",
            name.quoted(),
            ")"
        ]);

        /// Call the unbox function for the given type.
        fn unbox<'el>(var: Tokens<'el, Swift<'el>>, ty: &'el str) -> Tokens<'el, Swift<'el>> {
            toks!["unbox(", var, ", as: ", ty, ".self)"]
        }
    }

    /// Decode the given value.
    fn encode_value<'a>(
        &self,
        ty: &'a RpType,
        name: &'a str,
        var: Tokens<'a, Swift<'a>>,
    ) -> Result<Tokens<'a, Swift<'a>>> {
        use self::RpType::*;

        let encode = match *ty {
            DateTime => toks![self.formatter.clone(), "().string(from: ", var, ")"],
            Bytes => toks![var, ".base64EncodedString()"],
            Array { ref inner } => {
                let inner = self.encode_value(inner, name, "inner".into())?;
                toks![
                    "try encode_array(",
                    var,
                    ", name: ",
                    name.quoted(),
                    ", inner: { inner in ",
                    inner,
                    " })"
                ]
            }
            Map { ref value, .. } => {
                let value = self.encode_value(value, name, "value".into())?;

                toks![
                    "try encode_map(",
                    var,
                    ", name: ",
                    name.quoted(),
                    ", value: { value in ",
                    value,
                    " })"
                ]
            }
            Name { .. } => toks!["try ", var, ".encode()"],
            _ => var,
        };

        return Ok(encode.into());
    }

    // Setup a field initializer.
    pub fn encode_field<'a, A>(
        &self,
        field: &'a RpField,
        append: A,
    ) -> Result<Tokens<'a, Swift<'a>>>
    where
        A: Fn(Tokens<'a, Swift<'a>>) -> Tokens<'a, Swift<'a>>,
    {
        let mut t = Tokens::new();

        let ident = field.safe_ident();
        let name = field.name();

        if field.is_optional() {
            t.push({
                let mut t = Tokens::new();

                let value = self.encode_value(&field.ty, name, "value".into())?;

                t.push(toks!["if let value = self.", ident, " {",]);
                t.nested(append(value));
                t.push("}");

                t
            });
        } else {
            let value = self.encode_value(&field.ty, name, toks!["self.", ident])?;
            t.push(append(value));
        }

        Ok(t.join_line_spacing())
    }

    fn decode_field<'a, I>(
        &self,
        compiler: &Compiler,
        field: &'a RpField,
        index: I,
    ) -> Result<Tokens<'a, Swift<'a>>>
    where
        I: for<'b> Fn(&'b RpField, Cons<'b>) -> (Cons<'b>, Tokens<'b, Swift<'b>>),
    {
        let mut t = Tokens::new();

        let ident = field.safe_ident();
        let f_ident = Rc::new(format!("f_{}", field.ident()));

        let (name, index) = index(field, Cons::from("json"));

        if field.is_optional() {
            t.push({
                let ty = compiler.into_field(field)?;

                let mut t = Tokens::new();

                t.push(toks!["var ", ident, ": ", ty, " = Optional.none"]);

                t.push({
                    let mut t = Tokens::new();

                    t.push(toks!["if let value = ", index, " {",]);

                    t.nested({
                        let mut t = Tokens::new();

                        let value = self.decode_value(compiler, &field.ty, name, "value".into())?;

                        t.push(toks![ident, " = Optional.some(", value, ")"]);

                        t.join_line_spacing()
                    });

                    t.push("}");

                    t
                });

                t.join_line_spacing()
            });
        } else {
            t.push(GuardMissing(
                f_ident.clone().into(),
                toks![index],
                name.clone(),
            ));

            let value = self.decode_value(compiler, &field.ty, name, toks![f_ident])?;
            t.push(toks!["let ", ident, " = ", value]);
        }

        Ok(t.join_line_spacing())
    }

    fn type_index<'a>(field: &'a RpField, var: Cons<'a>) -> (Cons<'a>, Tokens<'a, Swift<'a>>) {
        let name = field.name();
        (
            Cons::from(name),
            toks![var, "[", Cons::from(name).quoted(), "]"],
        )
    }

    fn utils_package(&self) -> RpVersionedPackage {
        let package = RpPackage::new(vec!["ReprotoSimple_Utils".to_string()]);
        RpVersionedPackage::new(package, None)
    }

    fn utils<'el>(&self) -> Result<FileSpec<'el>> {
        let mut out = FileSpec::default();

        let numerics = vec!["Int", "UInt", "Int32", "Int64", "UInt32", "UInt64"];
        let floats = vec!["Float", "Double"];
        let simple = vec!["String", "Bool"];

        out.0.push({
            let mut t = Tokens::new();

            t.push("enum SerializationError: Error {");
            t.nested("case missing(String)");
            t.nested("case invalid(String)");
            t.nested("case bad_value()");
            t.push("}");

            t
        });

        out.0.push(decode_name_func());
        out.0.push(decode_value_func());

        for ty in numerics.iter().chain(floats.iter()).cloned() {
            out.0.push(unbox_number_func(ty, &numerics, &floats));
        }

        for ty in simple.iter().cloned() {
            out.0.push(unbox_simple_func(ty));
        }

        out.0.push(decode_array_func());
        out.0.push(encode_array_func());
        out.0.push(decode_map_func());
        out.0.push(encode_map_func());

        return Ok(out);

        /// Build a simple unboxing functions.
        fn unbox_simple_func<'el>(ty: &'el str) -> Tokens<'el, Swift<'el>> {
            let mut t = Tokens::new();

            t.push(toks![
                "func unbox(_ value: Any, as type: ",
                ty,
                ".Type) -> ",
                ty,
                "? {"
            ]);
            t.nested(toks!["return value as? ", ty]);
            t.push("}");

            t
        }

        /// Build an integer unboxing function.
        ///
        /// This is more complicated since it needs to handle numeric conversions.
        fn unbox_number_func<'el>(
            ty: &'el str,
            numerics: &[&'el str],
            floats: &[&'el str],
        ) -> Tokens<'el, Swift<'el>> {
            let mut t = Tokens::new();

            t.push(toks![
                "func unbox(_ value: Any, as type: ",
                ty,
                ".Type) -> ",
                ty,
                "? {"
            ]);
            t.nested({
                let mut t = Tokens::new();

                t.push("switch value {");

                t.nested({
                    let mut t = Tokens::new();

                    for ck in numerics.iter().cloned() {
                        if ty == ck {
                            continue;
                        }

                        t.push(toks!["case let n as ", ck, ":"]);
                        t.nested(toks!["return ", ty, "(exactly: n)"]);
                    }

                    for ck in floats.iter().cloned() {
                        if ty == ck {
                            continue;
                        }

                        t.push(toks!["case let n as ", ck, ":"]);
                        t.nested(toks!["return ", ty, "(n)"]);
                    }

                    t.push("default:");
                    t.nested(toks!["return value as? ", ty]);

                    t
                });

                t.push("}");

                t
            });

            t.push("}");
            t
        }

        /// Build an array decoding function.
        fn decode_array_func<'el>() -> Tokens<'el, Swift<'el>> {
            let mut t = Tokens::new();

            t.push(toks![
                "func decode_array<T>(_ value: Any, name: String, inner: (Any) throws -> T) \
                 throws -> [T] {"
            ]);
            t.nested({
                let mut t = Tokens::new();

                t.push("let array = try decode_name(value as? [Any], name: name)");
                t.push("var out = [T]()");
                t.push("for item in array {");
                t.nested("out.append(try inner(item))");
                t.push("}");
                t.push("return out");

                t
            });
            t.push("}");

            t
        }

        /// Build an array encoding function.
        fn encode_array_func<'el>() -> Tokens<'el, Swift<'el>> {
            let mut t = Tokens::new();

            t.push(toks![
                "func encode_array<T>(_ array: [T], name: String, inner: (T) throws -> Any) \
                 throws -> [Any] {"
            ]);
            t.nested({
                let mut t = Tokens::new();

                t.push("var out = [Any]()");
                t.push("for item in array {");
                t.nested("out.append(try inner(item))");
                t.push("}");
                t.push("return out");

                t
            });
            t.push("}");

            t
        }

        /// Build an array decoding function.
        fn decode_map_func<'el>() -> Tokens<'el, Swift<'el>> {
            let mut t = Tokens::new();

            t.push(toks![
                "func decode_map<T>(_ map: Any, name: String, value: (Any) throws -> T) throws -> \
                 [String: T] {"
            ]);
            t.nested({
                let mut t = Tokens::new();

                t.push("let map = try decode_name(map as? [String: Any], name: name)");
                t.push("var out = [String: T]()");
                t.push("for (k, v) in map {");
                t.nested("out[k] = try value(v)");
                t.push("}");
                t.push("return out");

                t
            });
            t.push("}");

            t
        }

        /// Build an array encoding function.
        fn encode_map_func<'el>() -> Tokens<'el, Swift<'el>> {
            let mut t = Tokens::new();

            t.push(toks![
                "func encode_map<T>(_ map: [String: T], name: String, value: (T) throws -> Any) \
                 throws -> [String: Any] {"
            ]);
            t.nested({
                let mut t = Tokens::new();

                t.push("var out = [String: Any]()");
                t.push("for (k, v) in map {");
                t.nested("out[k] = try value(v)");
                t.push("}");
                t.push("return out");

                t
            });
            t.push("}");

            t
        }

        /// Build a generic decoding function with named errors.
        fn decode_name_func<'el>() -> Tokens<'el, Swift<'el>> {
            let mut t = Tokens::new();

            t.push(toks![
                "func decode_name<T>(_ unbox: T?, name string: String) throws -> T {"
            ]);
            t.nested({
                let mut t = Tokens::new();

                t.push(toks!["guard let value = unbox else {"]);
                t.nested("throw SerializationError.invalid(string)");
                t.push("}");

                t.push("return value");

                t
            });

            t.push("}");
            t
        }

        /// Build a generic decoding function.
        fn decode_value_func<'el>() -> Tokens<'el, Swift<'el>> {
            let mut t = Tokens::new();

            t.push(toks!["func decode_value<T>(_ value: T?) throws -> T {"]);
            t.nested({
                let mut t = Tokens::new();

                t.push(toks!["guard let value = value else {"]);
                t.nested("throw SerializationError.bad_value()");
                t.push("}");

                t.push("return value");

                t
            });

            t.push("}");
            t
        }
    }
}

impl TypeCodegen for Codegen {
    fn generate(&self, e: TypeAdded) -> Result<()> {
        let TypeAdded {
            container,
            compiler,
            name,
            fields,
        } = e;

        container.push({
            let mut t = Tokens::new();

            t.push(toks!["public extension ", name.clone(), " {"]);

            t.push({
                let mut t = Tokens::new();

                // decode function
                t.nested(decode(self, compiler, name.clone(), &fields)?);
                t.nested(encode(self, &fields)?);

                t.join_line_spacing()
            });

            t.push("}");
            t
        });

        return Ok(());

        fn decode<'a>(
            codegen: &Codegen,
            compiler: &Compiler,
            name: Tokens<'a, Swift<'a>>,
            fields: &[&'a RpField],
        ) -> Result<Tokens<'a, Swift<'a>>> {
            let mut t = Tokens::new();

            t.push(toks![
                "static func decode(json: Any) throws -> ",
                name.clone(),
                " {"
            ]);
            t.nested({
                let mut t = Tokens::new();
                let mut args = Tokens::new();

                if !fields.is_empty() {
                    t.push(toks!["let json = try decode_value(json as? [String: Any])"]);

                    for field in fields.iter().cloned() {
                        let ident = field.safe_ident();
                        t.push(codegen.decode_field(compiler, field, Codegen::type_index)?);
                        args.append(toks![ident.clone(), ": ", ident.clone()]);
                    }
                } else {
                    t.push(toks!["let _ = try decode_value(json as? [String: Any])"]);
                }

                t.push(toks!["return ", name.clone(), "(", args.join(", "), ")"]);
                t.join_line_spacing()
            });

            t.push("}");

            Ok(t)
        }

        fn encode<'a>(codegen: &Codegen, fields: &[&'a RpField]) -> Result<Tokens<'a, Swift<'a>>> {
            let mut t = Tokens::new();

            t.push("func encode() throws -> [String: Any] {");
            t.nested({
                let mut t = Tokens::new();

                if !fields.is_empty() {
                    t.push("var json = [String: Any]()");
                    t.push({
                        let mut t = Tokens::new();

                        for field in fields.iter().cloned() {
                            t.push(codegen.encode_field(field, |value| {
                                toks!["json[", field.name().quoted(), "] = ", value]
                            })?);
                        }

                        t
                    });
                    t.push("return json");
                } else {
                    t.push("return [String: Any]()");
                }

                t.join_line_spacing()
            });
            t.push("}");

            Ok(t)
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

        container.push({
            let mut t = Tokens::new();

            t.push(toks!["public extension ", name.clone(), " {"]);

            t.push({
                let mut t = Tokens::new();

                t.nested(decode(self, compiler, name.clone(), &fields)?);
                t.nested(encode(self, &fields)?);

                t.join_line_spacing()
            });

            t.push("}");
            t
        });

        return Ok(());

        fn decode<'a>(
            codegen: &Codegen,
            compiler: &Compiler,
            name: Tokens<'a, Swift<'a>>,
            fields: &[&'a RpField],
        ) -> Result<Tokens<'a, Swift<'a>>> {
            let mut t = Tokens::new();
            let mut args = Tokens::new();

            t.push(toks![
                "static func decode(json: Any) throws -> ",
                name.clone(),
                " {"
            ]);
            t.nested({
                let mut t = Tokens::new();

                t.push(toks!["let json = try decode_value(json as? [Any])"]);

                for (i, field) in fields.iter().cloned().enumerate() {
                    let ident = field.safe_ident();
                    t.push(codegen.decode_field(compiler, field, |_, var| {
                        let i = Cons::from(i.to_string());
                        (
                            Cons::from(format!("[{}]", i.as_ref())),
                            toks!["Optional.some(", var, "[", i, "])"],
                        )
                    })?);
                    args.append(toks![ident.clone(), ": ", ident.clone()]);
                }

                t.join_line_spacing()
            });

            t.nested(toks!["return ", name.clone(), "(", args.join(", "), ")"]);
            t.push("}");

            Ok(t)
        }

        fn encode<'el, 'a>(
            codegen: &Codegen,
            fields: &[&'a RpField],
        ) -> Result<Tokens<'a, Swift<'a>>> {
            let mut t = Tokens::new();

            t.push("func encode() throws -> [Any] {");
            t.nested({
                let mut t = Tokens::new();

                t.push("var json = [Any]()");
                t.push_unless_empty({
                    let mut t = Tokens::new();

                    for field in fields.iter().cloned() {
                        t.push(codegen.encode_field(field, |value| toks!["json.append(", value, ")"])?);
                    }

                    t
                });
                t.push("return json");

                t.join_line_spacing()
            });
            t.push("}");

            Ok(t)
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

        container.push({
            let mut t = Tokens::new();

            t.push(toks!["public extension ", name.clone(), " {"]);

            t.push({
                let mut t = Tokens::new();

                // decode function
                t.nested(decode(body, name.clone())?);
                t.nested(encode(body)?);

                t.join_line_spacing()
            });

            t.push("}");
            t
        });

        return Ok(());

        fn decode<'a>(
            body: &'a RpEnumBody,
            name: Tokens<'a, Swift<'a>>,
        ) -> Result<Tokens<'a, Swift<'a>>> {
            let mut t = Tokens::new();

            t.push(toks![
                "static func decode(json: Any) throws -> ",
                name.clone(),
                " {"
            ]);
            t.nested({
                let mut t = Tokens::new();

                t.push(toks!["let json = try decode_value(json as? String)"]);

                t.push({
                    let mut t = Tokens::new();

                    t.push("switch json {");

                    for variant in &body.variants {
                        t.nested({
                            let mut t = Tokens::new();
                            t.push(toks!["case ", variant.ordinal().quoted(), ":"]);
                            t.nested(toks![
                                "return ",
                                name.clone(),
                                ".",
                                variant.local_name.as_str(),
                            ]);
                            t
                        });
                    }

                    t.nested({
                        let mut t = Tokens::new();
                        t.push("default:");
                        t.nested("throw SerializationError.bad_value()");
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

        fn encode<'a>(body: &'a RpEnumBody) -> Result<Tokens<'a, Swift<'a>>> {
            let mut t = Tokens::new();

            t.push("func encode() throws -> String {");
            t.nested({
                let mut t = Tokens::new();

                t.push("switch self {");

                for variant in &body.variants {
                    t.nested({
                        let mut t = Tokens::new();
                        t.push(toks!["case .", variant.local_name.as_str(), ":"]);
                        t.nested(toks!["return ", variant.ordinal().quoted()]);
                        t
                    });
                }

                t.nested({
                    let mut t = Tokens::new();
                    t.push("default:");
                    t.nested("throw SerializationError.bad_value()");
                    t
                });

                t.push("}");

                t
            });
            t.push("}");

            Ok(t)
        }
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

        container.push({
            let mut t = Tokens::new();

            t.push_unless_empty(Comments(&body.comment));
            t.push(toks!["public extension ", name.clone(), " {"]);

            t.push({
                let mut t = Tokens::new();

                match body.sub_type_strategy {
                    RpSubTypeStrategy::Tagged { ref tag, .. } => {
                        // decode function
                        t.nested(decode_tag(
                            compiler,
                            name.clone(),
                            tag.as_str(),
                            &body.sub_types,
                        )?);
                        t.nested(encode_tag(tag.as_str(), &body.sub_types)?);
                    }
                }

                t.join_line_spacing()
            });

            t.push("}");
            t
        });

        return Ok(());

        /// Build a method to decode a tagged interface.
        fn decode_tag<'el, S>(
            compiler: &Compiler,
            name: Tokens<'el, Swift<'el>>,
            tag: &'el str,
            sub_types: S,
        ) -> Result<Tokens<'el, Swift<'el>>>
        where
            S: IntoIterator<Item = (&'el String, &'el Rc<Loc<RpSubType>>)>,
        {
            let mut t = Tokens::new();

            t.push(toks![
                "static func decode(json: Any) throws -> ",
                name.clone(),
                " {"
            ]);
            t.nested({
                let mut t = Tokens::new();

                t.push(toks!["let json = try decode_value(json as? [String: Any])"]);
                t.push(toks![
                    "let type = try decode_name(json[",
                    tag.quoted(),
                    "] as? String, name: ",
                    tag.quoted(),
                    ")"
                ]);

                t.push({
                    let mut t = Tokens::new();
                    t.push("switch type {");

                    for (_, sub_type) in sub_types.into_iter() {
                        let n = compiler.convert_name(&sub_type.name)?;

                        let local_name = sub_type.local_name.as_str();

                        t.nested({
                            let mut t = Tokens::new();
                            t.push(toks!["case ", sub_type.name().quoted(), ":"]);
                            t.nested(toks!["let v = try ", n.clone(), ".decode(json: json)"]);
                            t.nested(toks!["return ", name.clone(), ".", local_name, "(v)"]);
                            t
                        });
                    }

                    t.nested({
                        let mut t = Tokens::new();

                        t.push("default:");
                        t.nested("throw SerializationError.invalid(type)");

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

        /// Build a method to decode a tagged interface.
        fn encode_tag<'el, S>(tag: &'el str, sub_types: S) -> Result<Tokens<'el, Swift<'el>>>
        where
            S: IntoIterator<Item = (&'el String, &'el Rc<Loc<RpSubType>>)>,
        {
            let mut t = Tokens::new();

            t.push(toks!["func encode() throws -> [String: Any] {"]);
            t.nested({
                let mut t = Tokens::new();
                t.push("switch self {");

                for (_, sub_type) in sub_types.into_iter() {
                    let name = sub_type.name();
                    let local_name = sub_type.local_name.as_str();

                    t.nested({
                        let mut t = Tokens::new();
                        t.push(toks!["case .", local_name, "(let s):"]);
                        t.nested(toks!["var json = try s.encode()"]);
                        t.nested(toks!["json[", tag.quoted(), "] = ", name.quoted()]);
                        t.nested(toks!["return json"]);
                        t
                    });
                }

                t.push("}");
                t
            });
            t.push("}");

            Ok(t)
        }
    }
}

impl PackageCodegen for Codegen {
    fn generate(&self, e: PackageAdded) -> Result<()> {
        let PackageAdded { files, .. } = e;
        files.insert(self.utils_package(), self.utils()?);
        Ok(())
    }
}
