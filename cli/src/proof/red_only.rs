//! `skies proof record <spec> --red-only`: rerun red alone and rewrite only the receipt's red half.
//!
//! `verify` never reruns red, so a red that no longer holds goes unnoticed until someone looks: a retro-spec's
//! red.patch stops applying once the code under it moves (`proof status` says `red-rotted`), or the red revision is
//! simply not the one intended. This is the fix path: regenerate the patch (or pick the revision), then rerun red
//! with the spec's red.patch, `--red-patch <file>`, or `--red <rev>`. Green, the footprint, the inputs, and green's
//! evidence (with their recorded hashes) are kept as they are.

use anyhow::{Result, bail};

use super::evidence::{self, Half};
use super::git::Repo;
use super::green::join;
use super::receipt::Receipt;
use super::record::{self, Options};
use super::report::FmId;
use super::runner::{Session, seconds};
use super::spec::{SpecDir, SpecDoc};
use crate::manifest::Project;

pub fn record(project: &Project, spec: &SpecDir, doc: &SpecDoc, repo: &Repo, options: &Options) -> Result<u8> {
    if options.with_impacted {
        bail!("--red-only reruns red alone; --with-impacted re-proves green, so run them separately");
    }
    let Some(mut receipt) = Receipt::load(spec)? else {
        bail!(
            "{} has no receipt yet, so there is no green to keep; `skies proof record {}` proves red and green together",
            spec.name,
            spec.id
        );
    };
    let unproven: Vec<FmId> = doc
        .failure_modes
        .iter()
        .filter(|id| !receipt.green.cases.contains_key(id))
        .copied()
        .collect();
    if !unproven.is_empty() {
        bail!(
            "{} added after recording, so green never ran for them; run `skies proof record {}` without --red-only",
            join(&unproven),
            spec.id
        );
    }
    let root = project.root.as_path();
    let runner_name = doc.runner(spec)?;
    let runner = project.runner(runner_name)?;
    evidence::ensure_ignored(root)?;
    let plan = record::plan_red(project, spec, repo, options)?;

    let scratch = tempfile::Builder::new().prefix("skies-proof-").tempdir()?;
    let mut session = Session::default();
    println!(
        "record {} red only (runner {runner_name}; green {} kept)",
        spec.name,
        record::short(&receipt.green.commit)
    );
    let Some((red_run, elapsed)) = record::prove_red(
        repo,
        spec,
        doc,
        (runner_name, runner),
        &plan,
        &mut session,
        scratch.path(),
    )?
    else {
        return Ok(1);
    };
    println!("  time   red {} (checkout and run)", seconds(elapsed));
    record::print_cases(&red_run.cases, "kept");

    let files = red_run.files(doc);
    if let Some(problem) = evidence::oversized(repo, spec, &files)? {
        eprintln!("{}: {problem}", spec.name);
        return Ok(1);
    }
    evidence::publish(spec, Half::Red, &files, &red_run.scrub)?;
    receipt.red = red_run.into_receipt(spec, plan.base.commit.clone(), plan.patch.as_deref());
    receipt.evidence = Some(evidence::hashes(
        spec,
        receipt.evidence.as_ref(),
        Some(Half::Green),
        &[],
    )?);
    receipt.save(spec)?;
    println!(
        "rewrote the red half of {}/receipt.json (green, footprint, inputs, and green evidence kept)",
        spec.rel()
    );
    Ok(0)
}
