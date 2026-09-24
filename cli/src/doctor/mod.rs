//! `skies doctor`: runs each platform's architecture analyzers and reports them together.

use anyhow::{Result, bail};

pub fn run(build_args: &[String]) -> Result<u8> {
    let _ = build_args;
    bail!("skies doctor: not implemented yet")
}
