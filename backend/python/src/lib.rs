#[macro_use]
extern crate log;
#[macro_use]
extern crate codeviz_macros;
extern crate codeviz_python;
extern crate reproto_backend as backend;

mod listeners;
mod python_backend;
mod python_compiler;
mod python_field;
mod python_file_spec;
mod python_options;

pub(crate) use self::listeners::*;
pub(crate) use self::python_backend::*;
pub(crate) use self::python_compiler::*;
pub(crate) use self::python_field::*;
pub(crate) use self::python_file_spec::*;
pub(crate) use self::python_options::*;
pub(crate) use backend::errors::*;
pub(crate) use backend::imports::*;
pub(crate) use codeviz_python::*;

pub(crate) const TYPE: &str = "type";
pub(crate) const INIT_PY: &str = "__init__.py";
pub(crate) const EXT: &str = "py";
pub(crate) const PYTHON_CONTEXT: &str = "python";

fn setup_module(module: &str) -> Result<Box<Listeners>> {
    let _module: Box<Listeners> = match module {
        _ => return Err(format!("No such module: {}", module).into()),
    };
}

pub fn setup_listeners(modules: Vec<String>) -> Result<(PythonOptions, Box<Listeners>)> {
    let mut listeners: Vec<Box<Listeners>> = Vec::new();

    for module in modules {
        listeners.push(setup_module(module.as_str())?);
    }

    let mut options = PythonOptions::new();

    for listener in &listeners {
        listener.configure(&mut options)?;
    }

    Ok((options, Box::new(listeners)))
}

pub fn compile_options<'a, 'b>(out: App<'a, 'b>) -> App<'a, 'b> {
    out.about("Compile for Python")
}

pub fn verify_options<'a, 'b>(out: App<'a, 'b>) -> App<'a, 'b> {
    out.about("Verify for Python")
}

pub fn compile(
    env: Environment,
    opts: Options,
    compiler_options: CompilerOptions,
    _matches: &ArgMatches,
) -> Result<()> {
    let id_converter = opts.id_converter;
    let (options, listeners) = setup_listeners(opts.modules)?;
    let backend = PythonBackend::new(env, options, listeners, id_converter);
    let compiler = backend.compiler(compiler_options)?;
    compiler.compile()
}

pub fn verify(env: Environment, opts: Options, _matches: &ArgMatches) -> Result<()> {
    let id_converter = opts.id_converter;
    let (options, listeners) = setup_listeners(opts.modules)?;
    let backend = PythonBackend::new(env, options, listeners, id_converter);
    backend.verify()
}
