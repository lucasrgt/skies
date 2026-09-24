//! Specs and receipts: `skies spec new`, `skies proof record|status|verify`.
//!
//! A feature is delivered with a receipt in its spec folder: its failure modes failed on a revision without the
//! feature (red) and pass on the working tree (green). The receipt is a record, not a turnstile; nothing here runs
//! in a hook or blocks anything by default.

mod git;
mod hash;
mod receipt;
mod record;
mod report;
mod runner;
mod spec;
mod verify;

use anyhow::{Context, Result, bail};

use crate::manifest::{FILE_NAME, Project};
use receipt::Freshness;

pub use record::record;
pub use verify::verify;

pub fn spec_new(slug: &str, runner: Option<&str>) -> Result<u8> {
    spec::validate_slug(slug)?;
    let project = Project::from_cwd()?;
    let runners: Vec<&str> = project.manifest.runners.keys().map(String::as_str).collect();
    let runner = match (runner, runners.as_slice()) {
        (Some(name), _) => {
            project.runner(name)?;
            name
        }
        (None, [only]) => only,
        (None, []) => bail!(
            "{FILE_NAME} declares no runner; add one (e.g. [runners.api] command = \"dotnet test ... --logger trx;LogFileName={{report}}\") or pass --runner"
        ),
        (None, many) => bail!("{FILE_NAME} declares several runners; pick one with --runner ({})", many.join(", ")),
    };

    let id = spec::next_id(&project.root)?;
    let name = format!("{id}-{slug}");
    let dir = project.root.join(spec::SPECS_DIR).join(&name);
    std::fs::create_dir_all(dir.join(spec::E2E_DIR)).with_context(|| format!("creating {}", dir.display()))?;
    std::fs::write(dir.join(spec::SPEC_FILE), spec::template(&id, slug, runner))
        .with_context(|| format!("writing {}", dir.join(spec::SPEC_FILE).display()))?;

    println!("created {}/{name}/ ({}, {}/)", spec::SPECS_DIR, spec::SPEC_FILE, spec::E2E_DIR);
    println!("next: list the failure modes in spec.md, then write one e2e case per mode titled \"FM-n: ...\"");
    Ok(0)
}

pub fn status() -> Result<u8> {
    let project = Project::from_cwd()?;
    let root = project.root.as_path();
    let specs = spec::discover(root)?;
    if specs.is_empty() {
        println!("no specs under {}/ (create one with `skies spec new <slug>`)", spec::SPECS_DIR);
        return Ok(0);
    }
    let width = specs.iter().map(|spec| spec.name.len()).max().unwrap_or(0);
    let mut unreadable = false;
    for spec in &specs {
        let line = match receipt::freshness(root, spec) {
            Ok(Freshness::Missing) => "no receipt".to_string(),
            Ok(Freshness::Current) => "current".to_string(),
            Ok(Freshness::Stale(changed)) => stale_line(&changed),
            Err(error) => {
                unreadable = true;
                format!("unreadable ({error:#})")
            }
        };
        println!("{:<width$}  {line}", spec.name);
    }
    Ok(if unreadable { 2 } else { 0 })
}

/// `stale (3 files changed: a, b, c)`, naming at most a handful so one line stays one line.
fn stale_line(changed: &[String]) -> String {
    const SHOWN: usize = 4;
    let noun = if changed.len() == 1 { "file" } else { "files" };
    let mut names = changed.iter().take(SHOWN).map(String::as_str).collect::<Vec<_>>().join(", ");
    if changed.len() > SHOWN {
        names.push_str(", …");
    }
    format!("stale ({} {noun} changed: {names})", changed.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_lines_stay_short() {
        assert_eq!(stale_line(&["a".into()]), "stale (1 file changed: a)");
        let many: Vec<String> = ["a", "b", "c", "d", "e"].map(String::from).to_vec();
        assert_eq!(stale_line(&many), "stale (5 files changed: a, b, c, d, …)");
    }
}
