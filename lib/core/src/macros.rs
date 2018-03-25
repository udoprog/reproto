/// Build a declaration body including common fields.
macro_rules! decl_body {
    (pub struct $name:ident<$f:ident> { $($rest:tt)* }) => {
        #[derive(Debug, Clone, Serialize)]
        pub struct $name<$f: 'static> where $f: $crate::flavor::Flavor {
            pub name: $crate::rp_name::RpName,
            pub ident: String,
            pub comment: Vec<String>,
            pub decls: Vec<$crate::rp_decl::RpDecl<$f>>,
            $($rest)*
        }
    };
}

#[macro_export]
macro_rules! decl_flavor {
    ($flavor: ident, $source: ident) => {
        pub type RpAccept = $source::RpAccept;
        pub type RpCode = $source::RpCode;
        pub type RpContext = $source::RpContext;
        pub type RpDecl = $source::RpDecl<$flavor>;
        pub type RpEndpoint = $source::RpEndpoint<$flavor>;
        pub type RpEndpointArgument = $source::RpEndpointArgument<$flavor>;
        pub type RpEndpointHttp = $source::RpEndpointHttp<$flavor>;
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
        pub type RpEnumOrdinal = $source::RpEnumOrdinal;
        pub type RpEnumType = $source::RpEnumType;
        pub type RpName = $source::RpName;
        pub type RpNumber = $source::RpNumber;
        pub type RpPackage = $source::RpPackage;
        pub type RpRequiredPackage = $source::RpRequiredPackage;
        pub type RpServiceBody = $source::RpServiceBody<$flavor>;
        pub type RpServiceBodyHttp = $source::RpServiceBodyHttp;
        pub type RpSubTypeStrategy = $source::RpSubTypeStrategy;
        pub type RpType = $source::RpType;
        pub type RpValue = $source::RpValue;
        pub type RpVariant = $source::RpVariant;
        pub type RpVersionedPackage = $source::RpVersionedPackage;
    };
}

/// Implement a Serialize that fails when trying to serializer for the given type.
#[macro_export]
macro_rules! no_serializer {
    ($ty: ident < $lifetime: tt >) => {
        impl<$lifetime> $crate::serde::Serialize for $ty<$lifetime> {
            fn serialize<S>(&self, serializer: S) -> ::std::result::Result<S::Ok, S::Error>
            where
                S: $crate::serde::Serializer,
            {
                Err($crate::serde::ser::Error::custom("not supported"))
            }
        }
    };
}
