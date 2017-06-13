use backend::*;
use backend::for_context::ForContext;
use codeviz::java::*;
use naming::{self, FromNaming};
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use super::converter::Converter;
pub use super::listeners::*;
use super::models::*;
use super::value_builder::*;
use super::variables::Variables;

const JAVA_CONTEXT: &str = "java";

pub struct ProcessorOptions {
    /// Should fields be nullable?
    pub nullable: bool,
    /// Should the type be immutable?
    pub immutable: bool,
    /// Build setters?
    pub build_setters: bool,
    /// Build getters?
    pub build_getters: bool,
    /// Build a constructor?
    pub build_constructor: bool,
    /// Build a Object#hashCode() implementation.
    pub build_hash_code: bool,
    /// Build a Object#equals() implementation.
    pub build_equals: bool,
    /// Build a Object#toString() implementation.
    pub build_to_string: bool,
}

impl ProcessorOptions {
    pub fn new() -> ProcessorOptions {
        ProcessorOptions {
            nullable: false,
            immutable: true,
            build_setters: true,
            build_getters: true,
            build_constructor: true,
            build_hash_code: true,
            build_equals: true,
            build_to_string: true,
        }
    }
}

pub struct Processor {
    options: ProcessorOptions,
    env: Environment,
    out_path: PathBuf,
    listeners: Box<Listeners>,
    package_prefix: Option<RpPackage>,
    snake_to_upper_camel: Box<naming::Naming>,
    snake_to_lower_camel: Box<naming::Naming>,
    null_string: Variable,
    suppress_warnings: ClassType,
    string_builder: ClassType,
    override_: ClassType,
    objects: ClassType,
    object: ClassType,
    list: ClassType,
    map: ClassType,
    string: ClassType,
    optional: ClassType,
    illegal_argument: ClassType,
    immutable_list: ClassType,
}

impl Processor {
    pub fn new(options: ProcessorOptions,
               env: Environment,
               out_path: PathBuf,
               package_prefix: Option<RpPackage>,
               listeners: Box<Listeners>)
               -> Processor {
        Processor {
            options: options,
            env: env,
            out_path: out_path,
            package_prefix: package_prefix,
            snake_to_upper_camel: naming::SnakeCase::new().to_upper_camel(),
            snake_to_lower_camel: naming::SnakeCase::new().to_lower_camel(),
            null_string: Variable::String("null".to_owned()),
            listeners: listeners,
            override_: Type::class("java.lang", "Override"),
            objects: Type::class("java.util", "Objects"),
            suppress_warnings: Type::class("java.lang", "SuppressWarnings"),
            string_builder: Type::class("java.lang", "StringBuilder"),
            object: Type::class("java.lang", "Object"),
            list: Type::class("java.util", "List"),
            map: Type::class("java.util", "Map"),
            string: Type::class("java.lang", "String"),
            optional: Type::class("java.util", "Optional"),
            illegal_argument: Type::class("java.lang", "IllegalArgumentException"),
            immutable_list: Type::class("com.google.common.collect", "ImmutableList"),
        }
    }

    fn field_mods(&self) -> Modifiers {
        let mut modifiers = mods![Modifier::Private];

        if self.options.immutable {
            modifiers.insert(Modifier::Final);
        }

        modifiers
    }

    /// Create a new FileSpec from the given package.
    fn new_file_spec(&self, pkg: &RpPackage) -> FileSpec {
        FileSpec::new(&self.java_package_name(pkg))
    }

    fn new_field_spec(&self, ty: &Type, name: &str) -> FieldSpec {
        let mods = self.field_mods();
        FieldSpec::new(mods, ty, name)
    }

    /// Build the java package of a given package.
    ///
    /// This includes the prefixed configured in `self.options`, if specified.
    fn java_package(&self, pkg: &RpPackage) -> RpPackage {
        self.package_prefix
            .clone()
            .map(|prefix| prefix.join(pkg))
            .unwrap_or_else(|| pkg.clone())
    }

    fn java_package_name(&self, pkg: &RpPackage) -> String {
        self.java_package(pkg).parts.join(".")
    }

    fn convert_custom(&self, pos: &RpPos, pkg: &RpPackage, name: &RpName) -> Result<Type> {
        let pkg = if let Some(ref prefix) = name.prefix {
            self.env
                .lookup_used(pkg, prefix)
                .map_err(|e| Error::pos(e.description().to_owned(), pos.clone()))?
        } else {
            pkg
        };

        let name = name.parts.join(".");

        let package_name = self.java_package_name(pkg);
        Ok(Type::class(&package_name, &name).into())
    }

    /// Convert the given type to a java type.
    pub fn into_java_type(&self, pos: &RpPos, pkg: &RpPackage, ty: &RpType) -> Result<Type> {
        let ty = match *ty {
            RpType::String => self.string.clone().into(),
            RpType::Signed(ref size) |
            RpType::Unsigned(ref size) => {
                // default to integer if unspecified.
                // TODO: should we care about signedness?
                // TODO: > 64 bits, use BitInteger?
                if size.map(|s| s <= 32usize).unwrap_or(true) {
                    INTEGER.into()
                } else {
                    LONG.into()
                }
            }
            RpType::Float => FLOAT.into(),
            RpType::Double => DOUBLE.into(),
            RpType::Boolean => BOOLEAN.into(),
            RpType::Array(ref ty) => {
                let argument = self.into_java_type(pos, pkg, ty)?;
                self.list.with_arguments(vec![argument]).into()
            }
            RpType::Name(ref name) => {
                return self.convert_custom(pos, pkg, name);
            }
            RpType::Map(ref key, ref value) => {
                let key = self.into_java_type(pos, pkg, key)?;
                let value = self.into_java_type(pos, pkg, value)?;
                self.map.with_arguments(vec![key, value]).into()
            }
            RpType::Any => self.object.clone().into(),
            ref t => {
                return Err(Error::pos(format!("unsupported type: {:?}", t), pos.clone()));
            }
        };

        Ok(ty)
    }

    fn build_constructor<C>(&self, class: &C) -> ConstructorSpec
        where C: ClassLike
    {
        let mut constructor = ConstructorSpec::new(mods![Modifier::Public]);

        for field in class.fields() {
            let argument = ArgumentSpec::new(mods![Modifier::Final], &field.ty, &field.name);
            constructor.push_argument(&argument);

            if !self.options.nullable {
                if let Some(non_null) = self.require_non_null(&field, &argument) {
                    constructor.push(non_null);
                }
            }

            constructor.push(stmt!["this.", &field.name, " = ", argument, ";"]);
        }

        constructor
    }

    /// Build a require-non-null check.
    fn require_non_null(&self, field: &FieldSpec, argument: &ArgumentSpec) -> Option<Statement> {
        match field.ty {
            Type::Primitive(_) => None,
            _ => {
                let require_non_null = stmt![&self.objects, ".requireNonNull"];
                let string = Variable::String(field.name.clone());
                Some(stmt![require_non_null, "(", &argument, ", ", string, ");"])
            }
        }
    }

    fn build_hash_code<C>(&self, class: &C) -> MethodSpec
        where C: ClassLike
    {
        let mut hash_code = MethodSpec::new(mods![Modifier::Public], "hashCode");

        hash_code.push_annotation(&self.override_);
        hash_code.returns(INTEGER);

        let mut method_body = Elements::new();

        method_body.push("int result = 1;");

        for field in class.fields() {
            let field_stmt = stmt!["this.", field];

            let value = match field.ty {
                Type::Primitive(ref primitive) => {
                    if *primitive == INTEGER {
                        field_stmt.clone()
                    } else {
                        stmt![primitive.as_boxed(), ".hashCode(", &field_stmt, ")"]
                    }
                }
                _ => stmt![&field_stmt, ".hashCode()"],
            };

            let value = if self.options.nullable {
                match field.ty {
                    Type::Primitive(_) => value,
                    _ => stmt!["(", &field_stmt, " != null ? 0 : ", value, ")"],
                }
            } else {
                value
            };

            method_body.push(stmt!["result = result * 31 + ", value, ";"]);
        }

        method_body.push("return result;");

        hash_code.push(&method_body);

        hash_code
    }

    fn build_equals<C>(&self, class_type: &ClassType, class: &C) -> MethodSpec
        where C: ClassLike
    {
        let mut equals = MethodSpec::new(mods![Modifier::Public], "equals");

        equals.push_annotation(&self.override_);
        equals.returns(BOOLEAN);

        let argument = ArgumentSpec::new(mods![Modifier::Final], &self.object, "other");

        equals.push_argument(&argument);

        // check if argument is null.
        {
            let mut null_check = Elements::new();

            null_check.push(stmt!["if (", &argument, " == null) {"]);
            null_check.push_nested("return false;");
            null_check.push("}");

            equals.push(null_check);
        }

        // check that argument is expected type.
        {
            let mut instanceof_check = Elements::new();

            instanceof_check.push(stmt!["if (!(", &argument, " instanceof ", class_type, ")) {"]);
            instanceof_check.push_nested("return false;");
            instanceof_check.push("}");

            equals.push(instanceof_check);
        }

        // cast argument.
        let o = stmt!["o"];

        let mut cast = Elements::new();

        let mut suppress_warnings = AnnotationSpec::new(&self.suppress_warnings);
        suppress_warnings.push_argument(Variable::String("unchecked".to_owned()));

        cast.push(suppress_warnings);
        cast.push(stmt!["final ", class_type, " ", &o, " = (", class_type, ") ", argument, ";"]);

        equals.push(cast);

        for field in class.fields() {
            let field_stmt = stmt!["this.", field];
            let o = stmt![&o, ".", &field.name];

            let equals_condition = match field.ty {
                Type::Primitive(_) => stmt![&field_stmt, " != ", &o],
                _ => stmt!["!", &field_stmt, ".equals(", &o, ")"],
            };

            let mut equals_check = Elements::new();

            equals_check.push(stmt!["if (", equals_condition, ") {"]);
            equals_check.push_nested("return false;");
            equals_check.push("}");

            if self.options.nullable {
                let mut null_check = Elements::new();

                null_check.push(stmt!["if (", &o, " != null) {"]);
                null_check.push_nested("return false;");
                null_check.push("}");

                let mut field_check = Elements::new();

                field_check.push(stmt!["if (", &field_stmt, " == null) {"]);
                field_check.push_nested(null_check);
                field_check.push("} else {");
                field_check.push_nested(equals_check);
                field_check.push("}");

                equals.push(field_check);
            } else {
                equals.push(equals_check);
            }
        }

        equals.push("return true;");

        equals
    }

    fn build_to_string<C>(&self, class_type: &ClassType, class: &C) -> MethodSpec
        where C: ClassLike
    {
        let mut to_string = MethodSpec::new(mods![Modifier::Public], "toString");

        to_string.push_annotation(&self.override_);
        to_string.returns(&self.string);

        let b = stmt!["b"];

        let new_string_builder = stmt!["new ", &self.string_builder, "();"];

        to_string.push(stmt!["final ", &self.string_builder, " ", &b, " = ", &new_string_builder]);

        let mut fields = Elements::new();

        for field in class.fields() {
            let mut field_append = Elements::new();

            let field_stmt = stmt!["this.", field];

            let format = match field.ty {
                Type::Primitive(ref primitive) => {
                    stmt![primitive.as_boxed(), ".toString(", &field_stmt, ")"]
                }
                _ => {
                    let format = stmt![&field_stmt, ".toString()"];

                    if self.options.nullable {
                        stmt![&field_stmt, " == null ? ", &self.null_string, " : ", format]
                    } else {
                        format
                    }
                }
            };

            let field_key = Variable::String(format!("{}=", &field.name));

            field_append.push(stmt![&b, ".append(", field_key, ");"]);
            field_append.push(stmt![&b, ".append(", format, ");"]);

            fields.push(field_append);
        }

        /// join each field with ", "
        let field_joiner = stmt![&b, ".append(", Variable::String(", ".to_owned()), ");"];

        let mut class_appends = Elements::new();

        class_appends.push(stmt![&b, ".append(", Variable::String(class_type.name.clone()), ");"]);
        class_appends.push(stmt![&b, ".append(", Variable::String("(".to_owned()), ");"]);
        class_appends.push(fields.join(field_joiner));
        class_appends.push(stmt![&b, ".append(", Variable::String(")".to_owned()), ");"]);

        to_string.push(class_appends);
        to_string.push(stmt!["return ", &b, ".toString();"]);

        to_string
    }

    fn add_class<C>(&self, class_type: &ClassType, class: &mut C) -> Result<()>
        where C: ClassLike + ContainerSpec
    {
        if self.options.build_constructor {
            let constructor = self.build_constructor(class);
            class.push_constructor(constructor);
        }

        if self.options.build_hash_code {
            let hash_code = self.build_hash_code(class);
            class.push(hash_code);
        }

        if self.options.build_equals {
            let equals = self.build_equals(class_type, class);
            class.push(equals);
        }

        if self.options.build_to_string {
            let to_string = self.build_to_string(class_type, class);
            class.push(to_string);
        }

        Ok(())
    }

    fn build_enum_constructor(&self, en: &EnumSpec) -> ConstructorSpec {
        let mut constructor = ConstructorSpec::new(mods![Modifier::Private]);

        for field in &en.fields {
            let argument = ArgumentSpec::new(mods![Modifier::Final], &field.ty, &field.name);
            constructor.push_argument(&argument);

            if !self.options.nullable {
                if let Some(non_null) = self.require_non_null(&field, &argument) {
                    constructor.push(non_null);
                }
            }

            constructor.push(stmt!["this.", &field.name, " = ", argument, ";"]);
        }

        constructor
    }

    fn find_field(&self, fields: &Vec<JavaField>, name: &str) -> Option<JavaField> {
        for field in fields {
            if field.name == name {
                return Some(field.clone());
            }
        }

        None
    }

    fn enum_from_value_method(&self,
                              field: &JavaField,
                              class_type: &ClassType)
                              -> Result<MethodSpec> {
        let argument = ArgumentSpec::new(mods![Modifier::Final], &field.java_type, &field.name);

        let value = stmt!["value"];

        let cond = match field.java_type {
            Type::Primitive(_) => stmt![&value, ".", &field.name, " == ", &argument],
            _ => stmt![&value, ".", &field.name, ".equals(", &argument, ")"],
        };

        let mut return_matched = Elements::new();

        return_matched.push(stmt!["if (", &cond, ") {"]);
        return_matched.push_nested(stmt!["return ", &value, ";"]);
        return_matched.push("}");

        let mut value_loop = Elements::new();

        value_loop.push(stmt!["for (final ", class_type, " ", &value, " : ", "values()) {"]);

        value_loop.push_nested(return_matched);
        value_loop.push("}");

        let mut from_value = MethodSpec::new(mods![Modifier::Public, Modifier::Static],
                                             "fromValue");

        let argument_name = Variable::String(argument.name.clone());
        let throw = stmt!["throw new ", &self.illegal_argument, "(", argument_name, ");"];

        from_value.returns(class_type);
        from_value.push_argument(argument);
        from_value.push(value_loop);
        from_value.push(throw);

        Ok(from_value)
    }

    fn enum_to_value_method(&self, field: &JavaField) -> Result<MethodSpec> {
        let mut to_value = MethodSpec::new(mods![Modifier::Public], "toValue");

        to_value.returns(&field.java_type);
        to_value.push(stmt!["return this.", &field.name, ";"]);

        Ok(to_value)
    }

    fn process_enum(&self, pkg: &RpPackage, body: &RpEnumBody) -> Result<FileSpec> {
        let class_type = Type::class(&self.java_package_name(pkg), &body.name);

        let mut spec = EnumSpec::new(mods![Modifier::Public], &body.name);
        let fields = self.convert_fields(pkg, &body.fields)?;

        for field in &fields {
            spec.push_field(&field.java_spec);

            if self.options.build_getters {
                spec.push(field.getter()?);
            }

            if self.options.build_setters {
                if let Some(setter) = field.setter()? {
                    spec.push(setter);
                }
            }
        }

        for code in body.codes.for_context(JAVA_CONTEXT) {
            spec.push(code.inner.lines);
        }

        let variables = Variables::new();

        for variant in &body.variants {
            let mut enum_value = Elements::new();
            let mut enum_stmt = stmt![&*variant.name];

            if !variant.arguments.is_empty() {
                let mut value_arguments = Statement::new();

                for (value, field) in variant.arguments.iter().zip(fields.iter()) {
                    let env = ValueBuilderEnv {
                        value: &value,
                        package: pkg,
                        ty: Some(&field.ty),
                        variables: &variables,
                    };

                    value_arguments.push(self.value(&env)?);
                }

                enum_stmt.push(stmt!["(", value_arguments.join(", "), ")"]);
            }

            enum_value.push(enum_stmt);
            spec.push_value(enum_value);
        }

        if !fields.is_empty() {
            let constructor = self.build_enum_constructor(&spec);
            spec.push_constructor(constructor);
        }

        let mut from_value: Option<MethodSpec> = None;
        let mut to_value: Option<MethodSpec> = None;

        if let Some(ref s) = body.serialized_as {
            if let Some(field) = self.find_field(&fields, &s.inner) {
                from_value = Some(self.enum_from_value_method(&field, &class_type)?);
                to_value = Some(self.enum_to_value_method(&field)?);
            } else {
                return Err(Error::pos(format!("no field named: {}", s.inner), s.pos.clone()));
            }
        }

        self.listeners
            .enum_added(&mut EnumAdded {
                body: body,
                fields: &fields,
                class_type: &class_type,
                from_value: &mut from_value,
                to_value: &mut to_value,
                spec: &mut spec,
            })?;

        if let Some(from_value) = from_value {
            spec.push(from_value);
        }

        if let Some(to_value) = to_value {
            spec.push(to_value);
        }

        let mut file_spec = self.new_file_spec(pkg);
        file_spec.push(&spec);

        Ok(file_spec)
    }

    fn process_tuple(&self, pkg: &RpPackage, body: &RpTupleBody) -> Result<FileSpec> {
        let class_type = Type::class(&self.java_package_name(pkg), &body.name);
        let mut spec = ClassSpec::new(mods![Modifier::Public], &body.name);

        let fields = self.convert_fields(pkg, &body.fields)?;

        for field in &fields {
            spec.push_field(&field.java_spec);

            if self.options.build_getters {
                spec.push(field.getter()?);
            }

            if self.options.build_setters {
                if let Some(setter) = field.setter()? {
                    spec.push(setter);
                }
            }
        }

        for code in body.codes.for_context(JAVA_CONTEXT) {
            spec.push(code.inner.lines);
        }

        self.add_class(&class_type, &mut spec)?;

        self.listeners
            .tuple_added(&mut TupleAdded {
                fields: &fields,
                class_type: &class_type,
                spec: &mut spec,
            })?;

        let mut file_spec = self.new_file_spec(pkg);
        file_spec.push(&spec);

        Ok(file_spec)
    }

    fn process_type(&self, type_id: &RpTypeId, body: &RpTypeBody) -> Result<FileSpec> {
        let class_type = Type::class(&self.java_package_name(&type_id.package), &body.name);

        let mut spec = ClassSpec::new(mods![Modifier::Public], &body.name);
        let fields = self.convert_fields(&type_id.package, &body.fields)?;

        for field in &fields {
            spec.push_field(&field.java_spec);

            if self.options.build_getters {
                spec.push(field.getter()?);
            }

            if self.options.build_setters {
                if let Some(setter) = field.setter()? {
                    spec.push(setter);
                }
            }
        }

        for code in body.codes.for_context(JAVA_CONTEXT) {
            spec.push(code.inner.lines);
        }

        self.add_class(&class_type, &mut spec)?;

        self.listeners
            .class_added(&mut ClassAdded {
                processor: self,
                type_id: type_id,
                fields: &fields,
                class_type: &class_type,
                match_decl: &body.match_decl,
                spec: &mut spec,
            })?;

        let mut file_spec = self.new_file_spec(&type_id.package);
        file_spec.push(&spec);

        Ok(file_spec)
    }

    fn process_interface(&self,
                         type_id: &RpTypeId,
                         interface: &RpInterfaceBody)
                         -> Result<FileSpec> {
        let parent_type = Type::class(&self.java_package_name(&type_id.package), &interface.name);

        let mut interface_spec = InterfaceSpec::new(mods![Modifier::Public], &interface.name);
        let interface_fields = self.convert_fields(&type_id.package, &interface.fields)?;

        for code in interface.codes.for_context(JAVA_CONTEXT) {
            interface_spec.push(code.inner.lines);
        }

        if self.options.build_getters {
            for field in &interface_fields {
                interface_spec.push(field.getter_without_body()?);
            }
        }

        for (_, ref sub_type) in &interface.sub_types {
            let class_type = parent_type.extend(&sub_type.name);

            let mods = mods![Modifier::Public, Modifier::Static];
            let mut class = ClassSpec::new(mods, &sub_type.name);
            let mut fields = interface_fields.clone();
            fields.extend(self.convert_fields(&type_id.package, &sub_type.fields)?);

            for code in sub_type.codes.for_context(JAVA_CONTEXT) {
                class.push(code.inner.lines);
            }

            class.implements(&parent_type);

            // override methods for interface fields.
            if self.options.build_getters {
                for field in &interface_fields {
                    let mut getter = field.getter()?;
                    getter.push_annotation(&self.override_);
                    class.push(getter);
                }
            }

            let mut fields = interface_fields.clone();
            fields.extend(self.convert_fields(&type_id.package, &sub_type.inner.fields)?);

            for field in &fields {
                class.push_field(&field.java_spec);

                if self.options.build_getters {
                    class.push(field.getter()?);
                }

                if self.options.build_setters {
                    if let Some(setter) = field.setter()? {
                        class.push(setter);
                    }
                }
            }

            self.add_class(&class_type, &mut class)?;

            self.listeners
                .class_added(&mut ClassAdded {
                    processor: self,
                    type_id: type_id,
                    fields: &fields,
                    class_type: &class_type,
                    match_decl: &sub_type.match_decl,
                    spec: &mut class,
                })?;

            self.listeners
                .sub_type_added(&mut SubTypeAdded {
                    fields: &fields,
                    interface: interface,
                    sub_type: sub_type,
                    spec: &mut class,
                })?;

            interface_spec.push(&class);
        }

        let mut file_spec = self.new_file_spec(&type_id.package);

        self.listeners
            .interface_added(&mut InterfaceAdded {
                interface: interface,
                spec: &mut interface_spec,
            })?;

        file_spec.push(&interface_spec);
        Ok(file_spec)
    }

    fn convert_field(&self, pkg: &RpPackage, field: &RpLoc<RpField>) -> Result<JavaField> {
        let java_type = self.into_java_type(&field.pos, pkg, &field.ty)?;
        let camel_name = self.snake_to_upper_camel.convert(&field.name);
        let ident = self.snake_to_lower_camel.convert(&field.name);
        let java_spec = self.build_field_spec(&java_type, field)?;

        Ok(JavaField {
            modifier: field.modifier.clone(),
            name: field.name.clone(),
            ty: field.ty.clone(),
            camel_name: camel_name,
            ident: ident,
            java_type: java_type,
            java_spec: java_spec,
        })
    }


    fn convert_fields(&self,
                      pkg: &RpPackage,
                      fields: &Vec<RpLoc<RpField>>)
                      -> Result<Vec<JavaField>> {
        let mut out = Vec::new();

        for field in fields {
            out.push(self.convert_field(pkg, field)?);
        }

        Ok(out)
    }

    fn build_field_spec(&self, field_type: &Type, field: &RpField) -> Result<FieldSpec> {
        let field_type = if field.is_optional() {
            self.optional.with_arguments(vec![field_type]).into()
        } else {
            field_type.clone()
        };

        let ident = self.snake_to_lower_camel.convert(&field.name);
        Ok(self.new_field_spec(&field_type, &ident))
    }

    fn process_files<F>(&self, mut consumer: F) -> Result<()>
        where F: FnMut(PathBuf, &RpTypeId, &RpDecl) -> Result<()>
    {
        let root_dir = &self.out_path;

        // Process all types discovered so far.
        for (ref type_id, ref decl) in &self.env.decls {
            let out_dir = self.java_package(&type_id.package)
                .parts
                .iter()
                .fold(root_dir.clone(), |current, next| current.join(next));

            let full_path = out_dir.join(format!("{}.java", decl.name()));
            consumer(full_path, type_id, decl)?;
        }

        Ok(())
    }

    fn build_file_spec(&self, type_id: &RpTypeId, decl: &RpDecl) -> Result<FileSpec> {
        match *decl {
            RpDecl::Interface(ref interface) => self.process_interface(type_id, interface),
            RpDecl::Type(ref ty) => self.process_type(type_id, ty),
            RpDecl::Tuple(ref ty) => self.process_tuple(&type_id.package, ty),
            RpDecl::Enum(ref ty) => self.process_enum(&type_id.package, ty),
        }
    }
}

impl Backend for Processor {
    fn process(&self) -> Result<()> {
        self.process_files(|full_path, type_id, decl| {
            debug!("+class: {}", full_path.display());

            if let Some(out_dir) = full_path.parent() {
                if !out_dir.is_dir() {
                    debug!("+dir: {}", out_dir.display());
                    fs::create_dir_all(&out_dir)?;
                }
            }

            let file_spec = self.build_file_spec(type_id, decl)?;

            let mut out = String::new();
            file_spec.format(&mut out)?;

            let mut f = File::create(full_path)?;
            f.write_all(&out.into_bytes())?;
            f.flush()?;

            Ok(())
        })
    }

    fn verify(&self) -> Result<Vec<Error>> {
        let mut errors = Vec::new();

        self.process_files(|_, type_id, decl| {
                match self.build_file_spec(type_id, decl) {
                    Err(e) => errors.push(e),
                    _ => {}
                };

                Ok(())
            })?;

        Ok(errors)
    }
}

impl Converter for Processor {
    type Type = Type;
    type Stmt = Statement;
    type Elements = Elements;
    type Variable = Variable;

    fn new_var(&self, name: &str) -> Self::Stmt {
        stmt![name]
    }

    fn convert_type(&self, pos: &RpPos, type_id: &RpTypeId) -> Result<Type> {
        let pkg = &type_id.package;
        let name = &type_id.name;

        let pkg = if let Some(ref prefix) = name.prefix {
            self.env
                .lookup_used(pkg, prefix)
                .map_err(|e| Error::pos(e.description().to_owned(), pos.clone()))?
        } else {
            pkg
        };

        let name = name.parts.join(".");

        let package_name = self.java_package_name(pkg);
        Ok(Type::class(&package_name, &name).into())
    }
}

/// Build values in python.
impl ValueBuilder for Processor {
    fn env(&self) -> &Environment {
        &self.env
    }

    fn identifier(&self, identifier: &str) -> Result<Self::Stmt> {
        Ok(stmt![identifier])
    }

    fn optional_empty(&self) -> Result<Self::Stmt> {
        Ok(stmt![&self.optional, ".empty()"])
    }

    fn constant(&self, ty: Self::Type) -> Result<Self::Stmt> {
        return Ok(stmt![ty]);
    }

    fn instance(&self, ty: Self::Type, arguments: Vec<Self::Stmt>) -> Result<Self::Stmt> {
        let mut stmt = Statement::new();

        for a in arguments {
            stmt.push(a);
        }

        Ok(stmt!["new ", &ty, "(", stmt.join(", "), ")"])
    }

    fn number(&self, number: &RpNumber) -> Result<Self::Stmt> {
        Ok(stmt![number.to_string()])
    }

    fn signed(&self, number: &RpNumber, size: &Option<usize>) -> Result<Self::Stmt> {
        let ty: Variable = if size.map(|s| s <= 32usize).unwrap_or(true) {
            format!("{}", number.to_string()).into()
        } else {
            format!("{}L", number.to_string()).into()
        };

        Ok(ty.into())
    }

    fn unsigned(&self, number: &RpNumber, size: &Option<usize>) -> Result<Self::Stmt> {
        let ty: Variable = if size.map(|s| s <= 32usize).unwrap_or(true) {
            format!("{}", number.to_string()).into()
        } else {
            format!("{}L", number.to_string()).into()
        };

        Ok(ty.into())
    }

    fn float(&self, number: &RpNumber) -> Result<Self::Stmt> {
        Ok(stmt![format!("{}F", number.to_string())])
    }

    fn double(&self, number: &RpNumber) -> Result<Self::Stmt> {
        Ok(stmt![format!("{}D", number.to_string())])
    }

    fn boolean(&self, boolean: &bool) -> Result<Self::Stmt> {
        Ok(stmt![boolean.to_string()])
    }

    fn string(&self, string: &str) -> Result<Self::Stmt> {
        Ok(Variable::String(string.to_owned()).into())
    }

    fn array(&self, values: Vec<Self::Stmt>) -> Result<Self::Stmt> {
        let mut arguments = Statement::new();

        for v in values {
            arguments.push(v);
        }

        Ok(stmt![&self.immutable_list, ".of(", arguments.join(", "), ")"])
    }
}
