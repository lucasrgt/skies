//! `skies proof verify`: rerun green for chosen specs and refresh their receipts.
//!
//! Red is never rerun here. It was established once, at the revision without the feature; what drifts afterwards is
//! the code under the feature, so verify re-proves green and re-anchors the hashes to today's files.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Result, bail};

use super::coverage;
use super::footprint::{self, Source};
use super::git::Repo;
use super::green::{self, GreenOutcome, ProvenGreen};
use super::hash;
use super::receipt::{self, Freshness, Receipt};
use super::report::FmId;
use super::runner::Session;
use super::scrub::Scrub;
use super::spec::{self, RECEIPT_FILE, SpecDir, SpecDoc};
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
    let outcomes = verify_specs(&project, &selected)?;
    print_outcomes(&outcomes);
    let failed = outcomes.iter().filter(|outcome| outcome.result.is_err()).count();
    if failed > 0 {
        println!("{failed} of {} failed; their receipts are unchanged", outcomes.len());
        return Ok(1);
    }
    Ok(0)
}

/// One spec's verify: `Ok(receipt hash)` when every failure mode passed and the receipt was refreshed, or why not.
pub struct Outcome {
    pub name: String,
    pub result: Result<Verified, String>,
}

pub struct Verified {
    pub modes: usize,
    /// The refreshed footprint in a few words (`footprint 14 files, coverage`), so a fallback to the diff shows.
    pub footprint: String,
    /// The blake3 of the refreshed receipt.json, which names exactly the receipt state that was proven.
    pub receipt: String,
}

/// Reruns green for each spec in order, sharing one session so a runner's setup runs once.
pub fn verify_specs(project: &Project, specs: &[SpecDir]) -> Result<Vec<Outcome>> {
    let root = project.root.as_path();
    let repo = Repo::open(root)?;
    let head = repo.head()?;
    let scratch = tempfile::Builder::new().prefix("skies-proof-").tempdir()?;
    let mut session = Session::default();
    let mut outcomes = Vec::new();
    for spec in specs {
        let result = match verify_one(root, spec, project, &repo, &head, &mut session, scratch.path()) {
            Ok(Ok((modes, footprint))) => Ok(Verified {
                modes,
                footprint,
                receipt: hash::hash_file(&spec.file(RECEIPT_FILE)).unwrap_or_else(|| hash::ABSENT.to_string()),
            }),
            Ok(Err(finding)) => Err(finding),
            Err(error) => Err(format!("{error:#}")),
        };
        outcomes.push(Outcome {
            name: spec.name.clone(),
            result,
        });
    }
    Ok(outcomes)
}

/// `<spec>  verified  n/n FMs pass`, or `failed` with the reason indented under it.
pub fn print_outcomes(outcomes: &[Outcome]) {
    let width = outcomes.iter().map(|outcome| outcome.name.len()).max().unwrap_or(0);
    for outcome in outcomes {
        let (verdict, detail) = match &outcome.result {
            Ok(verified) => (
                "verified",
                format!("{0}/{0} FMs pass ({1})", verified.modes, verified.footprint),
            ),
            Err(finding) => ("failed  ", finding.clone()),
        };
        let mut lines = detail.lines();
        println!(
            "{:<width$}  {verdict}  {}",
            outcome.name,
            lines.next().unwrap_or_default()
        );
        for line in lines {
            println!("{:<width$}            {line}", "");
        }
    }
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
    Ok(spec::discover(root)?
        .into_iter()
        .filter(|spec| chosen.contains(&spec.name))
        .collect())
}

/// `Ok(Ok((n, footprint)))` when all n failure modes pass and the receipt was refreshed; `Ok(Err(why))` when the run does not
/// prove the spec; `Err` when it could not run at all.
fn verify_one(
    root: &Path,
    spec: &SpecDir,
    project: &Project,
    repo: &Repo,
    head: &str,
    session: &mut Session,
    scratch: &Path,
) -> Result<Result<(usize, String), String>> {
    let Some(mut receipt) = Receipt::load(spec)? else {
        return Ok(Err(format!("no receipt; run `skies proof record {}` first", spec.name)));
    };
    let doc = SpecDoc::load(spec)?;
    let unrecorded: Vec<FmId> = doc
        .failure_modes
        .iter()
        .filter(|id| !receipt.red.cases.contains_key(id))
        .copied()
        .collect();
    if !unrecorded.is_empty() {
        return Ok(Err(format!(
            "{} added after recording, so red never ran for them; run `skies proof record {}`",
            green::join(&unrecorded),
            spec.name
        )));
    }

    let proven = match green::run_green(root, spec, &doc, project, session, scratch)? {
        GreenOutcome::Proven(proven) => proven,
        GreenOutcome::Refuted(message) => return Ok(Err(message)),
    };
    green::publish_evidence(
        spec,
        &proven.staged,
        &[(proven.file.clone(), proven.report.clone())],
        true,
        &Scrub::new(&[&repo.top]),
    )?;

    refresh_footprint(root, &doc, &mut receipt, &proven)?;
    let count = proven.cases.len();
    receipt.runner = doc.runner(spec)?.to_string();
    receipt.green.commit = head.to_string();
    receipt.green.dirty = repo.dirty()?;
    receipt.green.cases = proven.cases;
    receipt.green.report = proven.report;
    receipt.inputs = hash::hash_all(root, &hash::input_paths(root, spec)?);
    receipt.evidence = Some(green::evidence_hashes(spec, receipt.evidence.as_ref())?);
    let footprint = format!(
        "footprint {}, {}",
        footprint::files(receipt.footprint.len()),
        match receipt.footprint_source {
            Source::Coverage => "coverage",
            Source::Diff => "diff",
        }
    );
    receipt.save(spec)?;
    Ok(Ok((count, footprint)))
}

/// Re-anchors the footprint to today's files. The changed part is never recomputed from git (that would sweep in
/// every commit since red): a coverage receipt keeps its recorded `footprint_changed`, and a diff receipt its
/// recorded files. When this green run wrote coverage, the executed part is replaced by what it executed today, so
/// the footprint follows the code as it evolves, and a diff receipt is upgraded to coverage. Without coverage the
/// recorded file set is kept as it was. `touches` is re-matched either way.
fn refresh_footprint(root: &Path, doc: &SpecDoc, receipt: &mut Receipt, proven: &ProvenGreen) -> Result<()> {
    let touched = hash::touched_paths(root, &doc.touches)?;
    let recorded: BTreeSet<String> = receipt.footprint.keys().cloned().collect();
    let paths = match (&proven.coverage, receipt.footprint_source) {
        (coverage::Outcome::Covered(_), source) => {
            let changed = match source {
                Source::Coverage => receipt.footprint_changed.clone(),
                Source::Diff => footprint::recorded_changed(&recorded, &touched),
            };
            let refreshed = footprint::build(root, doc, &changed, proven)?;
            receipt.footprint_source = refreshed.source;
            receipt.footprint_changed = refreshed.changed;
            refreshed.paths
        }
        (coverage::Outcome::Missing(_), _) => {
            let mut paths = recorded;
            paths.extend(touched);
            paths.retain(|path| !hash::is_spec_path(path));
            paths
        }
    };
    receipt.footprint = hash::hash_all(root, &paths);
    Ok(())
}
