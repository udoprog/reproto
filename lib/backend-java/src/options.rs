//! Options for java code generation.

use codegen::{ClassCodegen, Codegen, EnumCodegen, GetterCodegen, InterfaceCodegen, ServiceCodegen,
              TupleCodegen};
use genco::Java;

pub struct Options {
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
    /// Indicates that a module requires that io.reproto.Observer is present.
    pub uses_observer: bool,
    /// Container to use for asynchronous operations.
    pub async_container: Option<Java<'static>>,
    /// Do not generate methods in service interface.
    pub suppress_service_methods: bool,
    /// Hook to generate code called in the root of the declarations.
    pub root_generators: Vec<Box<Codegen>>,
    /// Hook to run getter generators.
    pub getter_generators: Vec<Box<GetterCodegen>>,
    /// Hook to run class generators.
    pub class_generators: Vec<Box<ClassCodegen>>,
    /// Hook to run service generators.
    pub service_generators: Vec<Box<ServiceCodegen>>,
    /// Hook to run tuple generators.
    pub tuple_generators: Vec<Box<TupleCodegen>>,
    /// Hook to run interface generators.
    pub interface_generators: Vec<Box<InterfaceCodegen>>,
    /// Hook to run enum generators.
    pub enum_generators: Vec<Box<EnumCodegen>>,
}

impl Options {
    pub fn new() -> Self {
        Self {
            nullable: false,
            immutable: true,
            build_setters: true,
            build_getters: true,
            build_constructor: true,
            build_hash_code: true,
            build_equals: true,
            build_to_string: true,
            uses_observer: false,
            async_container: None,
            suppress_service_methods: false,
            root_generators: Vec::new(),
            getter_generators: Vec::new(),
            class_generators: Vec::new(),
            service_generators: Vec::new(),
            tuple_generators: Vec::new(),
            interface_generators: Vec::new(),
            enum_generators: Vec::new(),
        }
    }
}
