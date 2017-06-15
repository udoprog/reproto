pub mod container;
pub mod converter;
pub mod decode;
pub mod doc;
pub mod dynamic_converter;
pub mod dynamic_decode;
pub mod dynamic_encode;
pub mod encode;
pub mod environment;
pub mod errors;
pub mod for_context;
pub mod java;
pub mod js;
pub mod json;
pub mod match_decode;
pub mod python;
pub mod value_builder;
pub mod variables;
mod package_processor;
mod collecting;

use options::Options;
pub use self::environment::{Environment, InitFields};
use self::errors::*;

pub trait Backend {
    fn process(&self) -> Result<()>;

    fn verify(&self) -> Result<Vec<Error>>;
}

pub fn resolve(backend: &str, options: Options, env: Environment) -> Result<Box<Backend>> {
    let backend: Box<Backend> = match backend {
        "java" => Box::new(java::resolve(options, env)?),
        "python" => Box::new(python::resolve(options, env)?),
        "js" => Box::new(js::resolve(options, env)?),
        "doc" => Box::new(doc::resolve(options, env)?),
        "json" => Box::new(json::resolve(options, env)?),
        _ => return Err(format!("Unknown backend type: {}", backend).into()),
    };

    Ok(backend)
}
