use ops::imports::*;
use std::env;

pub fn options<'a, 'b>() -> App<'a, 'b> {
    let out = SubCommand::with_name("manifest");
    let out = compiler_base(out).about("Dump manifest configuration");
    out
}

pub fn entry(_matches: &ArgMatches) -> Result<()> {
    let path = env::current_dir()?.join("reproto.toml");
    let manifest = read_manifest(path)?;
    println!("{:?}", manifest);
    Ok(())
}
