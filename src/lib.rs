#![recursion_limit = "1000"]

extern crate clap;
extern crate lalrpop_util;
extern crate linked_hash_map;
extern crate num;
extern crate pulldown_cmark;

#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate log;
#[macro_use]
extern crate codeviz;

pub mod backend;
pub mod commands;
pub mod errors;
pub mod loc;
pub mod logger;
pub mod naming;
pub mod options;
pub mod parser;
pub mod core;
