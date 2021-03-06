pub mod processor;

use backend::*;
use backend::models as m;
use options::Options;

fn setup_module(module: &str) -> Result<Box<processor::Listeners>> {
    let _module: Box<processor::Listeners> = match module {
        _ => return Err(format!("No such module: {}", module).into()),
    };
}

pub fn resolve(options: Options, env: Environment) -> Result<processor::Processor> {
    let package_prefix = options.package_prefix
        .clone()
        .map(|prefix| m::Package::new(prefix.split(".").map(ToOwned::to_owned).collect()));

    let mut listeners = Vec::new();

    for module in &options.modules {
        listeners.push(setup_module(module)?);
    }

    let mut options = processor::ProcessorOptions::new(options);

    for listener in &listeners {
        listener.configure(&mut options)?;
    }

    return Ok(processor::Processor::new(options, env, package_prefix, Box::new(listeners)));
}
