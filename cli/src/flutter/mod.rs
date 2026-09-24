//! Flutter tooling: app and feature scaffolds, the dart-dio client, and the SKYFL doctor rules.

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

pub fn app(name: &str, path: Option<&Path>) -> Result<u8> {
    let _ = path;
    bail!("skies g flutter-app {name}: not implemented yet")
}
