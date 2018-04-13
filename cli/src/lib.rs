#![recursion_limit = "1000"]

#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;

extern crate ansi_term;
extern crate clap;
extern crate genco;
#[cfg(feature = "notify")]
extern crate notify;
extern crate reproto_ast as ast;
extern crate reproto_backend as backend;
extern crate reproto_backend_doc as doc;
extern crate reproto_compile as compile;
extern crate reproto_core as core;
extern crate reproto_derive as derive;
extern crate reproto_env as env;
extern crate reproto_manifest as manifest;
extern crate reproto_parser as parser;
extern crate reproto_repository as repository;
extern crate reproto_semck as semck;
extern crate reproto_trans as trans;
extern crate toml;
extern crate url;

mod build_spec;
pub mod ops;
pub mod output;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
