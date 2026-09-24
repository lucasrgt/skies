//! React tooling: feature scaffold, typed client generation, and i18n assembly.

use std::path::Path;

use anyhow::{Result, bail};

pub fn feature(package: &Path, name: &str) -> Result<u8> {
    bail!(
        "skies g feature {name} for {}: not implemented yet",
        package.display()
    )
}

pub fn client(package: &Path) -> Result<u8> {
    bail!(
        "skies g client for {}: not implemented yet",
        package.display()
    )
}

pub fn i18n(package: Option<&Path>) -> Result<u8> {
    let _ = package;
    bail!("skies i18n: not implemented yet")
}
