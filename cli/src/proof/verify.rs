//! `skies proof verify`: rerun green for chosen specs and refresh their receipts.
//!
//! Red is never rerun here. It was established once, at the revision without the feature; what drifts afterwards is
//! the code under the feature, so verify re-proves green and re-anchors the hashes to today's files.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Result, bail};

use super::git::Repo;
use super::hash::{self, Hashes};
use super::receipt::{self, Freshness, Receipt};
use super::record::{self, GreenOutcome};
use super::report::FmId;
use super::runner::Session;
use super::spec::{self, SpecDir, SpecDoc};
use crate::manifest::Project;

pub fn verify(keys: &[String], stale: bool, all: bool) -> Result<u8> {
    let project = Project::from_cwd()?;
    let root = project.root.as_path();
    let selected = select(root, keys, stale, all)?;
    if selected.is_empty() {
        if keys.is_empty() && !all && !stale {
            bail!("nothing to verify: name specs (skies proof verify 0001 0002), or pass --stale or --all");
        }
        println!("nothing to verify: every receipt is current");
        return Ok(0);
    }

    let repo = Repo::open(root)?;
    let head = repo.head()?;
    let scratch = tempfile::Builder::new().prefix("skies-proof-").tempdir()?;
    let mut session = Session::default();
    let mut rows = Vec::new();
    for spec in &selected {
        let result = verify_one(root, spec, &project, &repo, &head, &mut session, scratch.path());
        rows.push(match result {
            Ok(Ok(count)) => (spec.name.clone(), true, format!("{count}/{count} FMs pass")),
            Ok(Err(finding)) => (spec.name.clone(), false, finding),
            Err(error) => (spec.name.clone(), false, format!("{error:#}")),
        });
    }

    let width = rows.iter().map(|(name, ..)| name.len()).max().unwrap_or(0);
    for (name, ok, detail) in &rows {
        let verdict = if *ok { "verified" } else { "failed  " };
        let mut lines = detail.lines();
        println!("{name:<width$}  {verdict}  {}", lines.next().unwrap_or_default());
        for line in lines {
            println!("{:<width$}            {line}", "");
        }
    }
    let failed = rows.iter().filter(|(_, ok, _)| !ok).count();
    if failed > 0 {
        println!("{failed} of {} failed; their receipts are unchanged", rows.len());
        return Ok(1);
    }
    Ok(0)
}

/// Named specs, then stale ones, then all, deduplicated, in id order.
fn select(root: &Path, keys: &[String], stale: bool, all: bool) -> Result<Vec<SpecDir>> {
    let mut chosen: BTreeSet<String> = BTreeSet::new();
    for key in keys {
        chosen.insert(spec::find(root, key)?.name);
    }
    for spec in spec::discover(root)? {
        let pick = all || (stale && matches!(receipt::freshness(root, &spec)?, Freshness::Stale(_)));
        if pick {
            chosen.insert(spec.name);
        }
    }
    Ok(spec::discover(root)?.into_iter().filter(|spec| chosen.contains(&spec.name)).collect())
}

/// `Ok(Ok(n))` when all n failure modes pass and the receipt was refreshed; `Ok(Err(why))` when the run does not
/// prove the spec; `Err` when it could not run at all.
fn verify_one(
    root: &Path,
    spec: &SpecDir,
    project: &Project,
    repo: &Repo,
    head: &str,
    session: &mut Session,
    scratch: &Path,
) -> Result<Result<usize, String>> {
    let Some(mut receipt) = Receipt::load(spec)? else {
        return Ok(Err(format!("no receipt; run `skies proof record {}` first", spec.name)));
    };
    let doc = SpecDoc::load(spec)?;
    let unrecorded: Vec<FmId> =
        doc.failure_modes.iter().filter(|id| !receipt.red.cases.contains_key(id)).copied().collect();
    if !unrecorded.is_empty() {
        return Ok(Err(format!(
            "{} added after recording, so red never ran for them; run `skies proof record {}`",
            record::join(&unrecorded),
            spec.name
        )));
    }

    let green = match record::run_green(root, spec, &doc, project, session, scratch)? {
        GreenOutcome::Proven(green) => green,
        GreenOutcome::Refuted(message) => return Ok(Err(message)),
    };
    record::publish_evidence(spec, &green.staged, &[(&green.file, &green.report)], true)?;

    let (footprint, inputs) = rehash(root, spec, &doc, &receipt.footprint)?;
    let count = green.cases.len();
    receipt.runner = doc.runner(spec)?.to_string();
    receipt.green.commit = head.to_string();
    receipt.green.dirty = repo.dirty()?;
    receipt.green.cases = green.cases;
    receipt.green.report = green.report;
    receipt.footprint = footprint;
    receipt.inputs = inputs;
    receipt.save(spec)?;
    Ok(Ok(count))
}

/// Keeps the recorded footprint's file set (recomputing it from git would sweep in every commit since red) and adds
/// whatever `touches` matches today; inputs are recollected so new e2e files are covered.
fn rehash(root: &Path, spec: &SpecDir, doc: &SpecDoc, footprint: &Hashes) -> Result<(Hashes, Hashes)> {
    let mut paths: BTreeSet<String> = footprint.keys().cloned().collect();
    paths.extend(hash::touched_paths(root, &doc.touches)?);
    paths.retain(|path| !hash::is_spec_path(path));
    let footprint: BTreeMap<String, String> = hash::hash_all(root, &paths);
    Ok((footprint, hash::hash_all(root, &hash::input_paths(root, spec)?)))
}
