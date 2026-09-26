//! Specs and receipts: `skies spec new`, `skies proof run|record|impact`.
//!
//! A feature is delivered with a receipt in its spec folder: its failure modes failed on a revision without the
//! feature (red) and passed on the working tree (green), once. Keeping green passing afterwards is CI's job (it
//! already runs every spec's cases on every push), so the engine never re-answers it: no staleness, no hashes, no
//! reruns. Red is the evidence nothing else produces. Nothing here runs in a hook or blocks anything by default.

mod avp;
mod avp_decision;
mod base;
mod evidence;
mod git;
mod grammar;
mod green;
mod guard;
mod impact;
mod receipt;
mod record;
mod red;
mod red_cause;
mod report;
mod run;
mod runner;
mod shell;
mod spec;
mod summary;

use anyhow::{Context, Result, bail};

use crate::manifest::{FILE_NAME, Project};

pub use impact::impact;
pub use record::record;
pub use run::run;

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
            "{FILE_NAME} declares no runner; add one (e.g. [runners.api] command = \"dotnet test ... --logger 'trx;LogFileName={{report}}'\"), quoting the logger so the shell does not end the command at its `;`"
        ),
        (None, many) => bail!(
            "{FILE_NAME} declares several runners; pick one with --runner ({})",
            many.join(", ")
        ),
    };

    let id = spec::next_id(&project.root)?;
    let name = format!("{id}-{slug}");
    let dir = project.root.join(spec::SPECS_DIR).join(&name);
    std::fs::create_dir_all(dir.join(spec::E2E_DIR)).with_context(|| format!("creating {}", dir.display()))?;
    std::fs::write(dir.join(spec::SPEC_FILE), spec::template(&id, slug, runner))
        .with_context(|| format!("writing {}", dir.join(spec::SPEC_FILE).display()))?;
    evidence::ensure_ignored(&project.root)?;

    println!(
        "created {}/{name}/ ({}, {}/)",
        spec::SPECS_DIR,
        spec::SPEC_FILE,
        spec::E2E_DIR
    );
    println!(
        "next: list the failure modes in spec.md as `- FM-<n> <what goes wrong>`, then write e2e cases titled \"FM-<n>: ...\""
    );
    Ok(0)
}
