//! .NET scaffolders: `skies new` and the backend `skies g` generators.

use anyhow::{Result, bail};

use crate::Generate;

pub fn new_app(name: &str) -> Result<u8> {
    bail!("skies new {name}: not implemented yet")
}

pub fn generate(command: Generate) -> Result<u8> {
    let _ = command;
    bail!("this generator is not implemented yet")
}
