/// Build a declaration body including common fields.
macro_rules! decl_body {
    (pub struct $name:ident<$f:ident> { $($rest:tt)* }) => {
        #[derive(Debug, Clone, Serialize)]
        #[serde(bound = "F: ::serde::Serialize, F::Field: ::serde::Serialize, F::Endpoint: ::serde::Serialize, F::Package: ::serde::Serialize, F::Name: ::serde::Serialize, F::EnumType: ::serde::Serialize")]
        pub struct $name<$f: 'static> where $f: $crate::flavor::Flavor {
            pub name: $f::Name,
            pub ident: String,
            pub comment: Vec<String>,
            pub decls: Vec<$crate::rp_decl::RpDecl<$f>>,
            $($rest)*
        }
    };
}

#[macro_export]
macro_rules! decl_flavor {
    ($flavor:ident, $source:ident) => {
        pub type RpAccept = $source::RpAccept;
        pub type RpCode = $source::RpCode;
        pub type RpContext = $source::RpContext;
        pub type RpDecl = $source::RpDecl<$flavor>;
        pub type RpEndpoint = $source::RpEndpoint<$flavor>;
        pub type RpEndpointArgument = $source::RpEndpointArgument<$flavor>;
        pub type RpEndpointHttp = $source::RpEndpointHttp<$flavor>;
        pub type RpEndpointHttp1 = $source::RpEndpointHttp1<$flavor>;
        pub type RpEnumBody = $source::RpEnumBody<$flavor>;
        pub type RpField = $source::RpField<$flavor>;
        pub type RpFile = $source::RpFile<$flavor>;
        pub type RpHttpMethod = $source::RpHttpMethod;
        pub type RpInterfaceBody = $source::RpInterfaceBody<$flavor>;
        pub type RpPathPart = $source::RpPathPart<$flavor>;
        pub type RpPathSpec = $source::RpPathSpec<$flavor>;
        pub type RpPathStep = $source::RpPathStep<$flavor>;
        pub type RpReg = $source::RpReg;
        pub type RpNamed<'a> = $source::RpNamed<'a, $flavor>;
        pub type RpSubType = $source::RpSubType<$flavor>;
        pub type RpTupleBody = $source::RpTupleBody<$flavor>;
        pub type RpTypeBody = $source::RpTypeBody<$flavor>;
        pub type RpChannel = $source::RpChannel<$flavor>;
        pub type RpEnumType = $source::RpEnumType;
        pub type RpName = $source::RpName<$flavor>;
        pub type RpNumber = $source::RpNumber;
        pub type RpPackage = $source::RpPackage;
        pub type RpRequiredPackage = $source::RpRequiredPackage;
        pub type RpServiceBody = $source::RpServiceBody<$flavor>;
        pub type RpServiceBodyHttp = $source::RpServiceBodyHttp;
        pub type RpSubTypeStrategy = $source::RpSubTypeStrategy;
        pub type RpType = $source::RpType<$flavor>;
        pub type RpValue = $source::RpValue;
        pub type RpVariant<V> = $source::RpVariant<$flavor, V>;
        pub type RpVariantRef<'a> = $source::RpVariantRef<'a, $flavor>;
        pub type RpVersionedPackage = $source::RpVersionedPackage;
    };
}

#[macro_export]
macro_rules! translator_defaults {
    ($slf:ident $($rest:tt)*) => {
        translator_defaults!(@internal $slf $($rest)*);
    };

    (@internal $slf:ident, local_name $($rest:tt)*) => {
        fn translate_local_name<T>(
            &self,
            translator: &T,
            _reg: $crate::RpReg,
            name: $crate::RpName<$slf::Source>,
        ) -> Result<$crate::RpName<$slf::Target>>
        where
            T: Translator<Source = $slf::Source, Target = $slf::Target>,
        {
            name.translate(translator)
        }

        translator_defaults!(@internal $slf $($rest)*);
    };

    (@internal $slf:ident, field $($rest:tt)*) => {
        fn translate_field<T>(
            &self,
            translator: &T,
            field: $crate::RpField<$slf::Source>,
        ) -> Result<$crate::RpField<$slf::Target>>
        where
            T: Translator<Source = $slf::Source, Target = $slf::Target>,
        {
            field.translate(translator)
        }

        translator_defaults!(@internal $slf $($rest)*);
    };

    (@internal $slf:ident, endpoint $($rest:tt)*) => {
        fn translate_endpoint<T>(
            &self,
            translator: &T,
            endpoint: $crate::RpEndpoint<$slf::Source>,
        ) -> Result<$crate::RpEndpoint<$slf::Target>>
        where
            T: Translator<Source = $slf::Source, Target = $slf::Target>,
        {
            endpoint.translate(translator)
        }

        translator_defaults!(@internal $slf $($rest)*);
    };

    (@internal $slf:ident, rp_type $($rest:tt)*) => {
        fn translate_i32(&self) -> Result<RpType<$slf::Target>> {
            Ok(RpType::Signed { size: 32 })
        }

        fn translate_i64(&self) -> Result<RpType<$slf::Target>> {
            Ok(RpType::Signed { size: 64 })
        }

        fn translate_u32(&self) -> Result<RpType<$slf::Target>> {
            Ok(RpType::Unsigned { size: 32 })
        }

        fn translate_u64(&self) -> Result<RpType<$slf::Target>> {
            Ok(RpType::Unsigned { size: 64 })
        }

        fn translate_float(&self) -> Result<RpType<$slf::Target>> {
            Ok(RpType::Float)
        }

        fn translate_double(&self) -> Result<RpType<$slf::Target>> {
            Ok(RpType::Double)
        }

        fn translate_boolean(&self) -> Result<RpType<$slf::Target>> {
            Ok(RpType::Boolean)
        }

        fn translate_string(&self) -> Result<RpType<$slf::Target>> {
            Ok(RpType::String)
        }

        fn translate_datetime(&self) -> Result<RpType<$slf::Target>> {
            Ok(RpType::DateTime)
        }

        fn translate_array(&self, inner: RpType<$slf::Target>) -> Result<RpType<$slf::Target>> {
            Ok(RpType::Array {
                inner: Box::new(inner),
            })
        }

        fn translate_map(
            &self,
            key: RpType<$slf::Target>,
            value: RpType<$slf::Target>,
        ) -> Result<RpType<$slf::Target>> {
            Ok(RpType::Map {
                key: Box::new(key),
                value: Box::new(value),
            })
        }

        fn translate_any(&self) -> Result<RpType<$slf::Target>> {
            Ok(RpType::Any)
        }

        fn translate_bytes(&self) -> Result<RpType<$slf::Target>> {
            Ok(RpType::Bytes)
        }

        fn translate_name(
            &self,
            _reg: RpReg,
            name: RpName<$slf::Target>,
        ) -> Result<<$slf::Target as Flavor>::Type> {
            Ok(RpType::Name { name })
        }

        translator_defaults!(@internal $slf $($rest)*);
    };

    (@internal $slf:ident, enum_type $($rest:tt)*) => {
        fn translate_enum_type<T>(
            &self,
            _: &T,
            enum_type: $crate::RpEnumType,
        ) -> Result<<$slf::Target as Flavor>::EnumType>
        where
            T: Translator<Source = $slf::Source, Target = $slf::Target>,
        {
            Ok(enum_type)
        }

        translator_defaults!(@internal $slf $($rest)*);
    };

    (@internal $slf:ident) => {
    };
}
