//! React tooling: feature scaffold, typed client generation, and i18n assembly.
//!
//! `scaffold` and `contract` are shared with the Flutter tooling: both platforms write files under the same
//! policies and find their backend contract through the same `Skies.toml` lookup.

mod client;
pub mod contract;
mod feature;
mod i18n;
pub mod names;
pub mod scaffold;

use std::path::Path;

use anyhow::Result;

pub fn feature(package: &Path, name: &str) -> Result<u8> {
    feature::scaffold(&scaffold::package_dir(Some(package))?, name)
}

pub fn client(package: &Path) -> Result<u8> {
    client::generate(&scaffold::package_dir(Some(package))?)
}

/// `skies i18n` serves both platforms: a package with `pubspec.yaml` assembles ARB catalogs, anything else
/// assembles the TypeScript resource module.
pub fn i18n(package: Option<&Path>) -> Result<u8> {
    let dir = scaffold::package_dir(package)?;
    if dir.join("pubspec.yaml").is_file() {
        crate::flutter::i18n(&dir)
    } else {
        i18n::assemble(&dir)
    }
}
