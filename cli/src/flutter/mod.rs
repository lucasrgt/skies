//! Flutter tooling: app and feature scaffolds, the dart-dio client, i18n assembly, and the SKYFL doctor rules.

mod app;
mod client;
mod feature;
mod i18n;
mod names;
mod projection;
pub mod rules;

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::web::scaffold::package_dir;

pub fn feature(package: &Path, name: &str) -> Result<u8> {
    feature::scaffold(&package_dir(Some(package))?, name)
}

pub use client::ClientOptions;

pub fn client(package: &Path, options: &ClientOptions) -> Result<u8> {
    client::generate(&package_dir(Some(package))?, options)
}

pub fn app(name: &str, path: Option<&Path>) -> Result<u8> {
    app::create(name, path)
}

pub fn i18n(package: &Path) -> Result<u8> {
    i18n::assemble(package)
}

/// Every file with extension `ext` under `root`, sorted, skipping Flutter's build output and every hidden folder
/// (`.dart_tool`, and `integration_test/.skies_spec/` where a spec runner copies a spec's cases to run them).
pub(crate) fn files_with_extension(root: &Path, ext: &str) -> Vec<PathBuf> {
    fn visit(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name != "build" && !name.starts_with('.') {
                    visit(&path, ext, out);
                }
            } else if path.extension().is_some_and(|e| e == ext) {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    visit(root, ext, &mut out);
    out.sort();
    out
}
