#![recursion_limit = "1000"]
#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate log;
extern crate codeviz_common;
extern crate reproto_core as core;
extern crate reproto_parser as parser;
extern crate reproto_repository as repository;
extern crate linked_hash_map;
extern crate clap;
extern crate serde_json;

mod into_model;
mod scope;
mod base_decode;
mod base_encode;
mod collecting;
mod container;
mod converter;
mod dynamic_converter;
mod dynamic_decode;
mod dynamic_encode;
mod environment;
mod for_context;
mod package_processor;
mod package_utils;
mod value_builder;
pub mod errors;
pub mod naming;
mod options;
pub mod imports;
mod compiler_options;

pub use self::compiler_options::CompilerOptions;
pub use self::environment::Environment;
pub use self::options::Options;
