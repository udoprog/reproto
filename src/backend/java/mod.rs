mod models;
pub mod builder;
pub mod constructor_properties;
pub mod fasterxml;
pub mod listeners;
pub mod lombok;
pub mod mutable;
pub mod nullable;
pub mod processor;

use backend::*;
use core::*;
use options::Options;

fn setup_module(module: &str) -> Result<Box<listeners::Listeners>> {
    let module: Box<listeners::Listeners> = match module {
        "builder" => Box::new(builder::Module::new()),
        "constructor_properties" => Box::new(constructor_properties::Module::new()),
        "fasterxml" => Box::new(fasterxml::Module::new()),
        "lombok" => Box::new(lombok::Module::new()),
        "mutable" => Box::new(mutable::Module::new()),
        "nullable" => Box::new(nullable::Module::new()),
        _ => return Err(format!("No such module: {}", module).into()),
    };

    Ok(module)
}

pub fn resolve(options: Options, env: Environment) -> Result<processor::Processor> {
    let out_path = options.out_path;

    let package_prefix = options.package_prefix
        .clone()
        .map(|prefix| RpPackage::new(prefix.split(".").map(ToOwned::to_owned).collect()));

    let mut listeners: Vec<Box<listeners::Listeners>> = Vec::new();

    for module in &options.modules {
        listeners.push(setup_module(module)?);
    }

    let mut options = processor::ProcessorOptions::new();

    for listener in &listeners {
        listener.configure(&mut options)?;
    }

    Ok(processor::Processor::new(options, env, out_path, package_prefix, Box::new(listeners)))
}
