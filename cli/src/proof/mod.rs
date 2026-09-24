//! Specs and receipts: `skies spec new`, `skies proof record|run|status|verify|impact`.
//!
//! A feature is delivered with a receipt in its spec folder: its failure modes failed on a revision without the
//! feature (red) and pass on the working tree (green). The receipt is a record, not a turnstile; nothing here runs
//! in a hook or blocks anything by default.

mod avp;
mod base;
mod coverage;
mod ctx;
mod evidence;
mod footprint;
mod git;
mod green;
mod hash;
mod impact;
mod lines;
mod receipt;
mod record;
mod red;
mod red_only;
mod report;
mod rot;
mod run;
mod runner;
mod scrub;
mod spec;
mod summary;
mod verify;

use anyhow::{Context, Result, bail};

use crate::manifest::{FILE_NAME, Project};
use receipt::Freshness;

pub use impact::impact;
pub use record::{Options as RecordOptions, record};
pub use run::run;
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

    println!(
        "created {}/{name}/ ({}, {}/)",
        spec::SPECS_DIR,
        spec::SPEC_FILE,
        spec::E2E_DIR
    );
    println!("next: list the failure modes in spec.md, then write one e2e case per mode titled \"FM-n: ...\"");
    Ok(0)
}

pub fn status() -> Result<u8> {
    let project = Project::from_cwd()?;
    let root = project.root.as_path();
    let specs = spec::discover(root)?;
    if specs.is_empty() {
        println!(
            "no specs under {}/ (create one with `skies spec new <slug>`)",
            spec::SPECS_DIR
        );
        return Ok(0);
    }
    let width = specs.iter().map(|spec| spec.name.len()).max().unwrap_or(0);
    // Every receipt is read once, and every footprint file shared between them is read and hashed once, in parallel.
    let receipts: Vec<Result<Option<receipt::Receipt>>> = specs.iter().map(receipt::Receipt::load).collect();
    let shared: std::collections::BTreeSet<&String> = receipts
        .iter()
        .flat_map(|receipt| receipt.as_ref().ok().and_then(Option::as_ref))
        .flat_map(|receipt| receipt.footprint.keys())
        .collect();
    let known = lines::Snapshot::read(root, shared);
    let committed_reports = receipts
        .iter()
        .filter(|receipt| matches!(receipt, Ok(Some(receipt)) if receipt.has_committed_reports()))
        .count();
    let rotted = rot::rotted(root, &specs);
    let mut unreadable = false;
    for (spec, receipt) in specs.iter().zip(receipts) {
        let (mut line, readable) =
            describe(receipt.and_then(|receipt| receipt::freshness_with(&project, spec, receipt, &known)));
        unreadable |= !readable;
        if rotted.contains(&spec.name) {
            line.push_str(&format!(", red-rotted ({} no longer applies)", spec::RED_PATCH_FILE));
        }
        println!("{:<width$}  {line}", spec.name);
    }
    if !rotted.is_empty() {
        println!(
            "red-rotted: the code under red.patch moved, so red can no longer be reproduced. Stub the feature out \
             again, save the diff as the spec's red.patch, restore the code, then `skies proof record <id> --red-only`."
        );
    }
    if committed_reports > 0 {
        println!(
            "{committed_reports} receipt{} still commit full reports under evidence/; `skies proof verify --refresh \
             --all` moves them to the local evidence/{}/ and keeps their summary in the receipt.",
            if committed_reports == 1 { "" } else { "s" },
            evidence::RAW_DIR
        );
    }
    Ok(if unreadable { 2 } else { 0 })
}

/// A receipt's standing in one phrase, and whether it could be read at all.
fn freshness_line(project: &Project, spec: &spec::SpecDir) -> (String, bool) {
    describe(receipt::freshness(project, spec))
}

fn describe(freshness: Result<Freshness>) -> (String, bool) {
    match freshness {
        Ok(Freshness::Missing) => ("unrecorded (no receipt)".to_string(), true),
        Ok(Freshness::Current) => ("current".to_string(), true),
        Ok(Freshness::Stale(changed)) => (changed_line("stale", "changed", &changed), true),
        Ok(Freshness::Tampered(edited)) => (changed_line("tampered", "edited since recording", &edited), true),
        Err(error) => (format!("unreadable ({error:#})"), false),
    }
}

/// `stale (3 files changed: a, b, c)`, naming at most a handful so one line stays one line.
fn changed_line(state: &str, verb: &str, changed: &[String]) -> String {
    const SHOWN: usize = 4;
    let noun = if changed.len() == 1 { "file" } else { "files" };
    let mut names = changed
        .iter()
        .take(SHOWN)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    if changed.len() > SHOWN {
        names.push_str(", …");
    }
    format!("{state} ({} {noun} {verb}: {names})", changed.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_lines_stay_short() {
        assert_eq!(
            changed_line("stale", "changed", &["a".into()]),
            "stale (1 file changed: a)"
        );
        let many: Vec<String> = ["a", "b", "c", "d", "e"].map(String::from).to_vec();
        assert_eq!(
            changed_line("stale", "changed", &many),
            "stale (5 files changed: a, b, c, d, …)"
        );
        assert_eq!(
            changed_line("tampered", "edited since recording", &["evidence/green.xml".into()]),
            "tampered (1 file edited since recording: evidence/green.xml)"
        );
    }
}
