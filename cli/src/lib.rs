#![recursion_limit = "1000"]

#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;

extern crate ansi_term;
extern crate clap;
extern crate genco;
extern crate reproto_ast as ast;
extern crate reproto_backend as backend;
extern crate reproto_backend_csharp as csharp;
extern crate reproto_backend_doc as doc;
extern crate reproto_backend_go as go;
extern crate reproto_backend_java as java;
extern crate reproto_backend_js as js;
extern crate reproto_backend_json as json;
extern crate reproto_backend_python as python;
extern crate reproto_backend_reproto as reproto;
extern crate reproto_backend_rust as rust;
extern crate reproto_backend_swift as swift;
extern crate reproto_compile as compile;
extern crate reproto_core as core;
extern crate reproto_derive as derive;
extern crate reproto_manifest as manifest;
extern crate reproto_parser as parser;
extern crate reproto_repository as repository;
extern crate reproto_repository_http as repository_http;
extern crate reproto_semck as semck;
extern crate reproto_trans as trans;
extern crate toml;
extern crate url;

pub mod ops;
pub mod config;
pub mod output;
mod build_spec;
mod config_env;
