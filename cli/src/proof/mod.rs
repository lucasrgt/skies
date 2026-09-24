//! Specs and receipts: `skies spec new`, `skies proof record|status|verify`.

use std::path::Path;

use anyhow::{Result, bail};

pub fn spec_new(slug: &str, runner: Option<&str>) -> Result<u8> {
    let _ = runner;
    bail!("skies spec new {slug}: not implemented yet")
}

pub fn record(spec: &str, red: Option<&str>, red_patch: Option<&Path>) -> Result<u8> {
    let _ = (red, red_patch);
    bail!("skies proof record {spec}: not implemented yet")
}

pub fn status() -> Result<u8> {
    bail!("skies proof status: not implemented yet")
}

pub fn verify(specs: &[String], stale: bool, all: bool) -> Result<u8> {
    let _ = (specs, stale, all);
    bail!("skies proof verify: not implemented yet")
}
