//! `skies migrate 5`: moves a 4.x application onto Skies 5.

use anyhow::{Result, bail};

pub fn run(version: u32, dry_run: bool) -> Result<u8> {
    let _ = dry_run;
    bail!("skies migrate {version}: not implemented yet")
}
