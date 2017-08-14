use super::*;

pub struct JavaBackend {
    pub env: Environment,
    options: JavaOptions,
    listeners: Box<Listeners>,
    snake_to_upper_camel: Box<Naming>,
    snake_to_lower_camel: Box<Naming>,
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

impl JavaBackend {
    pub fn new(env: Environment, options: JavaOptions, listeners: Box<Listeners>) -> JavaBackend {
        JavaBackend {
            env: env,
            options: options,
            snake_to_upper_camel: SnakeCase::new().to_upper_camel(),
            snake_to_lower_camel: SnakeCase::new().to_lower_camel(),
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

    pub fn compiler(&self, options: CompilerOptions) -> Result<JavaCompiler> {
        Ok(JavaCompiler {
            out_path: options.out_path,
            backend: self,
        })
    }

    pub fn verify(&self) -> Result<()> {
        Ok(())
    }

    fn field_mods(&self) -> Modifiers {
        let mut modifiers = mods![Modifier::Private];

        if self.options.immutable {
            modifiers.insert(Modifier::Final);
        }

        modifiers
    }

    /// Create a new FileSpec from the given package.
    fn new_file_spec(&self, pkg: &RpVersionedPackage) -> FileSpec {
        FileSpec::new(&self.java_package_name(pkg))
    }

    fn new_field_spec(&self, ty: &Type, name: &str) -> FieldSpec {
        let mods = self.field_mods();
        FieldSpec::new(mods, ty, name)
    }

    /// Build the java package of a given package.
    ///
    /// This includes the prefixed configured in `self.options`, if specified.
    pub fn java_package(&self, pkg: &RpVersionedPackage) -> RpPackage {
        pkg.into_package(|version| {
            format!("_{}", version).replace(".", "_").replace("-", "_")
        })
    }

    fn java_package_name(&self, pkg: &RpVersionedPackage) -> String {
        self.java_package(pkg).parts.join(".")
    }

    fn convert_type_id(&self, pos: &Pos, lookup_id: &RpTypeId) -> Result<Type> {
        let LookupResult {
            package,
            registered,
            type_id,
            ..
        } = self.env
            .lookup(&lookup_id.package, &lookup_id.name)
            .map_err(|e| Error::pos(e.description().to_owned(), pos.into()))?;

        let package_name = self.java_package_name(package);
        let name = registered.local_name(&type_id, |p| p.join("."), |c| c.join("."));

        Ok(Type::class(&package_name, &name).into())
    }

    /// Convert the given type to a java type.
    pub fn into_java_type(&self, pos: &Pos, ty: &RpType, type_id: &RpTypeId) -> Result<Type> {
        let ty: Type = match *ty {
            RpType::String => self.string.clone().into(),
            RpType::Signed { ref size } |
            RpType::Unsigned { ref size } => {
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
            RpType::Array { ref inner } => {
                let argument = self.into_java_type(pos, inner, type_id)?;
                self.list.with_arguments(vec![argument]).into()
            }
            RpType::Name { ref name } => {
                let type_id = type_id.with_name(name.clone());
                self.convert_type_id(pos, &type_id)?.into()
            }
            RpType::Map { ref key, ref value } => {
                let key = self.into_java_type(pos, key, type_id)?;
                let value = self.into_java_type(pos, value, type_id)?;
                self.map.with_arguments(vec![key, value]).into()
            }
            RpType::Any => self.object.clone().into(),
            ref t => {
                return Err(Error::pos(format!("unsupported type: {:?}", t), pos.into()));
            }
        };

        Ok(ty)
    }

    fn build_constructor<C>(&self, class: &C) -> ConstructorSpec
    where
        C: ClassLike,
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
    where
        C: ClassLike,
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
    where
        C: ClassLike,
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

            instanceof_check.push(stmt![
                "if (!(",
                &argument,
                " instanceof ",
                class_type,
                ")) {",
            ]);
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
        cast.push(stmt![
            "final ",
            class_type,
            " ",
            &o,
            " = (",
            class_type,
            ") ",
            argument,
            ";",
        ]);

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
    where
        C: ClassLike,
    {
        let mut to_string = MethodSpec::new(mods![Modifier::Public], "toString");

        to_string.push_annotation(&self.override_);
        to_string.returns(&self.string);

        let b = stmt!["b"];

        let new_string_builder = stmt!["new ", &self.string_builder, "();"];

        to_string.push(stmt![
            "final ",
            &self.string_builder,
            " ",
            &b,
            " = ",
            &new_string_builder,
        ]);

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

        class_appends.push(stmt![
            &b,
            ".append(",
            Variable::String(class_type.name.clone()),
            ");",
        ]);
        class_appends.push(stmt![
            &b,
            ".append(",
            Variable::String("(".to_owned()),
            ");",
        ]);
        class_appends.push(fields.join(field_joiner));
        class_appends.push(stmt![
            &b,
            ".append(",
            Variable::String(")".to_owned()),
            ");",
        ]);

        to_string.push(class_appends);
        to_string.push(stmt!["return ", &b, ".toString();"]);

        to_string
    }

    fn add_class<C>(&self, class_type: &ClassType, class: &mut C) -> Result<()>
    where
        C: ClassLike + ContainerSpec,
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

    fn find_field<'a>(&self, fields: &[JavaField<'a>], name: &str) -> Option<JavaField<'a>> {
        for field in fields {
            if field.name == name {
                return Some(field.clone());
            }
        }

        None
    }

    fn enum_from_value_method(
        &self,
        field: &JavaField,
        class_type: &ClassType,
    ) -> Result<MethodSpec> {
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

        value_loop.push(stmt![
            "for (final ",
            class_type,
            " ",
            &value,
            " : ",
            "values()) {",
        ]);

        value_loop.push_nested(return_matched);
        value_loop.push("}");

        let mut from_value =
            MethodSpec::new(mods![Modifier::Public, Modifier::Static], "fromValue");

        let argument_name = Variable::String(argument.name.clone());
        let throw =
            stmt![
            "throw new ",
            &self.illegal_argument,
            "(",
            argument_name,
            ");",
        ];

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

    fn process_enum(&self, type_id: &RpTypeId, body: &RpEnumBody) -> Result<EnumSpec> {
        let class_type = Type::class(&self.java_package_name(&type_id.package), &body.name);

        let mut spec = EnumSpec::new(mods![Modifier::Public], &body.name);
        let fields = self.convert_fields(type_id, &body.fields)?;

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
            spec.push(code.move_inner().lines);
        }

        let variables = Variables::new();

        for variant in &body.variants {
            let mut enum_value = Elements::new();
            let mut enum_stmt = stmt![&*variant.name];

            if !variant.arguments.is_empty() {
                let mut value_arguments = Statement::new();

                for (value, field) in variant.arguments.iter().zip(fields.iter()) {
                    let ctx =
                        ValueContext::new(&type_id.package, &variables, &value, Some(&field.ty));
                    value_arguments.push(self.value(ctx)?);
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
            if let Some(field) = self.find_field(&fields, s.as_ref()) {
                from_value = Some(self.enum_from_value_method(&field, &class_type)?);
                to_value = Some(self.enum_to_value_method(&field)?);
            } else {
                return Err(Error::pos(format!("no field named: {}", s), s.pos().into()));
            }
        }

        self.listeners.enum_added(&mut EnumAdded {
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

        Ok(spec)
    }

    fn process_tuple(&self, type_id: &RpTypeId, body: &RpTupleBody) -> Result<ClassSpec> {
        let class_type = Type::class(&self.java_package_name(&type_id.package), &body.name);
        let mut spec = ClassSpec::new(mods![Modifier::Public], &body.name);

        let fields = self.convert_fields(type_id, &body.fields)?;

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
            spec.push(code.move_inner().lines);
        }

        self.add_class(&class_type, &mut spec)?;

        self.listeners.tuple_added(&mut TupleAdded {
            fields: &fields,
            class_type: &class_type,
            spec: &mut spec,
        })?;

        Ok(spec)
    }

    fn process_type(&self, type_id: &RpTypeId, body: &RpTypeBody) -> Result<ClassSpec> {
        let class_type = Type::class(&self.java_package_name(&type_id.package), &body.name);

        let mut spec = ClassSpec::new(mods![Modifier::Public], &body.name);
        let fields = self.convert_fields(type_id, &body.fields)?;

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
            spec.push(code.move_inner().lines);
        }

        self.add_class(&class_type, &mut spec)?;

        self.listeners.class_added(&mut ClassAdded {
            backend: self,
            type_id: type_id,
            fields: &fields,
            class_type: &class_type,
            match_decl: &body.match_decl,
            spec: &mut spec,
        })?;

        Ok(spec)
    }

    fn process_interface(
        &self,
        type_id: &RpTypeId,
        interface: &RpInterfaceBody,
    ) -> Result<InterfaceSpec> {
        let parent_type = Type::class(&self.java_package_name(&type_id.package), &interface.name);

        let mut interface_spec = InterfaceSpec::new(mods![Modifier::Public], &interface.name);
        let interface_fields = self.convert_fields(type_id, &interface.fields)?;

        for code in interface.codes.for_context(JAVA_CONTEXT) {
            interface_spec.push(code.move_inner().lines);
        }

        if self.options.build_getters {
            for field in &interface_fields {
                interface_spec.push(field.getter_without_body()?);
            }
        }

        for (_, ref sub_type) in &interface.sub_types {
            let type_id = type_id.extend(sub_type.name.to_owned());

            let class_type = parent_type.extend(&sub_type.name);

            let mods = mods![Modifier::Public, Modifier::Static];
            let mut class = ClassSpec::new(mods, &sub_type.name);
            let sub_type_fields = self.convert_fields(&type_id, &sub_type.fields)?;

            for code in sub_type.codes.for_context(JAVA_CONTEXT) {
                class.push(code.move_inner().lines);
            }

            class.implements(&parent_type);

            // override methods for interface fields.
            for field in &interface_fields {
                if self.options.build_getters {
                    let mut getter = field.getter()?;
                    getter.push_annotation(&self.override_);
                    class.push(getter);
                }

                if self.options.build_setters {
                    if let Some(mut setter) = field.setter()? {
                        setter.push_annotation(&self.override_);
                        class.push(setter);
                    }
                }
            }

            for field in &sub_type_fields {
                if self.options.build_getters {
                    class.push(field.getter()?);
                }

                if self.options.build_setters {
                    if let Some(setter) = field.setter()? {
                        class.push(setter);
                    }
                }
            }

            let mut fields = interface_fields.clone();
            fields.extend(sub_type_fields);

            for field in &fields {
                class.push_field(&field.java_spec);
            }

            self.add_class(&class_type, &mut class)?;

            self.listeners.class_added(&mut ClassAdded {
                backend: self,
                type_id: &type_id,
                fields: &fields,
                class_type: &class_type,
                match_decl: &sub_type.match_decl,
                spec: &mut class,
            })?;

            self.listeners.sub_type_added(&mut SubTypeAdded {
                fields: &fields,
                interface: interface,
                sub_type: sub_type,
                spec: &mut class,
            })?;

            interface_spec.push(&class);
        }

        self.listeners.interface_added(&mut InterfaceAdded {
            interface: interface,
            spec: &mut interface_spec,
        })?;

        Ok(interface_spec)
    }

    fn process_service(&self, _type_id: &RpTypeId, body: &RpServiceBody) -> Result<InterfaceSpec> {
        Ok(InterfaceSpec::new(mods![Modifier::Public], &body.name))
    }

    fn convert_field<'a>(
        &self,
        type_id: &RpTypeId,
        field: &'a Loc<RpField>,
    ) -> Result<JavaField<'a>> {
        let java_value_type = self.into_java_type(field.pos(), &field.ty, type_id)?;

        let java_type = match field.is_optional() {
            true => {
                self.optional
                    .with_arguments(vec![java_value_type.clone()])
                    .into()
            }
            false => java_value_type.clone(),
        };

        let camel_name = self.snake_to_upper_camel.convert(field.ident());
        let ident = self.snake_to_lower_camel.convert(field.ident());
        let java_spec = self.build_field_spec(&java_type, field)?;

        Ok(JavaField {
            modifier: &field.modifier,
            name: field.name(),
            ty: &field.ty,
            camel_name: camel_name,
            ident: ident,
            java_value_type: java_value_type,
            java_type: java_type,
            java_spec: java_spec,
        })
    }

    fn convert_fields<'a>(
        &self,
        type_id: &RpTypeId,
        fields: &'a Vec<Loc<RpField>>,
    ) -> Result<Vec<JavaField<'a>>> {
        let mut out = Vec::new();

        for field in fields {
            out.push(self.convert_field(type_id, field)?);
        }

        Ok(out)
    }

    fn build_field_spec(&self, field_type: &Type, field: &RpField) -> Result<FieldSpec> {
        let ident = self.snake_to_lower_camel.convert(field.ident());
        Ok(self.new_field_spec(&field_type, &ident))
    }

    fn process_decl(
        &self,
        type_id: &RpTypeId,
        decl: &RpDecl,
        depth: usize,
        container: &mut Container,
    ) -> Result<()> {
        match *decl {
            RpDecl::Interface(ref interface) => {
                let mut spec = self.process_interface(type_id, interface)?;

                for d in &interface.decls {
                    let type_id = type_id.extend(d.name().to_owned());
                    self.process_decl(&type_id, d, depth + 1, &mut spec)?;
                }

                container.push_contained(spec.into());
            }
            RpDecl::Type(ref ty) => {
                let mut spec = self.process_type(type_id, ty)?;

                // Inner classes should be static.
                if depth > 0 {
                    spec.modifiers.insert(Modifier::Static);
                }

                for d in &ty.decls {
                    let type_id = type_id.extend(d.name().to_owned());
                    self.process_decl(&type_id, d, depth + 1, &mut spec)?;
                }

                container.push_contained(spec.into());
            }
            RpDecl::Tuple(ref ty) => {
                let mut spec = self.process_tuple(type_id, ty)?;

                // Inner classes should be static.
                if depth > 0 {
                    spec.modifiers.insert(Modifier::Static);
                }

                for d in &ty.decls {
                    let type_id = type_id.extend(d.name().to_owned());
                    self.process_decl(&type_id, d, depth + 1, &mut spec)?;
                }

                container.push_contained(spec.into());
            }
            RpDecl::Enum(ref ty) => {
                let mut spec = self.process_enum(type_id, ty)?;

                for d in &ty.decls {
                    let type_id = type_id.extend(d.name().to_owned());
                    self.process_decl(&type_id, d, depth + 1, &mut spec)?;
                }

                container.push_contained(spec.into());
            }
            RpDecl::Service(ref ty) => {
                container.push_contained(self.process_service(type_id, ty)?.into());
            }
        }

        Ok(())
    }

    pub fn build_file_spec(&self, type_id: &RpTypeId, decl: &RpDecl) -> Result<FileSpec> {
        let mut file_spec = self.new_file_spec(&type_id.package);
        self.process_decl(type_id, decl, 0usize, &mut file_spec)?;
        Ok(file_spec)
    }
}

impl Converter for JavaBackend {
    type Type = Type;
    type Stmt = Statement;
    type Elements = Elements;
    type Variable = Variable;

    fn new_var(&self, name: &str) -> Self::Stmt {
        stmt![name]
    }

    fn convert_type(&self, pos: &Pos, type_id: &RpTypeId) -> Result<Type> {
        self.convert_type_id(pos, type_id)
    }
}

/// Build values in python.
impl ValueBuilder for JavaBackend {
    fn env(&self) -> &Environment {
        &self.env
    }

    fn identifier(&self, identifier: &str) -> Result<Self::Stmt> {
        Ok(stmt![identifier])
    }

    fn optional_empty(&self) -> Result<Self::Stmt> {
        Ok(stmt![&self.optional, ".empty()"])
    }

    fn optional_of(&self, value: Self::Stmt) -> Result<Self::Stmt> {
        Ok(stmt![&self.optional, ".of(", value, ")"])
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

        Ok(stmt![
            &self.immutable_list,
            ".of(",
            arguments.join(", "),
            ")",
        ])
    }
}

trait Container {
    fn push_contained(&mut self, element: Element);
}

impl Container for FileSpec {
    fn push_contained(&mut self, element: Element) {
        self.push(element);
    }
}

impl Container for InterfaceSpec {
    fn push_contained(&mut self, element: Element) {
        self.push(element);
    }
}

impl Container for ClassSpec {
    fn push_contained(&mut self, element: Element) {
        self.push(element);
    }
}

impl Container for EnumSpec {
    fn push_contained(&mut self, element: Element) {
        self.push(element);
    }
}
