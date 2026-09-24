//! `skies proof verify`: rerun green for chosen specs, and refresh the receipts that need it.
//!
//! Red is never rerun here. It was established once, at the revision without the feature; what drifts afterwards is
//! the code under the feature, so verify re-proves green. A receipt that is current and still passes is left exactly
//! as it is (verify is then read-only, so a baseline run before a change dirties nothing); a stale one is
//! re-anchored to today's files, and `--refresh` re-anchors a current one too.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::Duration;

use anyhow::{Result, bail};

use super::coverage;
use super::footprint::{self, Source};
use super::git::Repo;
use super::green::{self, GreenOutcome, ProvenGreen};
use super::hash;
use super::lines;
use super::receipt::{self, Freshness, Receipt};
use super::report::FmId;
use super::runner::{Session, seconds};
use super::scrub::Scrub;
use super::spec::{self, RECEIPT_FILE, SpecDir, SpecDoc};
use crate::manifest::{Project, Runner};

pub fn verify(keys: &[String], stale: bool, all: bool, refresh: bool) -> Result<u8> {
    let project = Project::from_cwd()?;
    let selected = select(&project, keys, stale, all)?;
    if selected.is_empty() {
        if keys.is_empty() && !all && !stale {
            bail!("nothing to verify: name specs (skies proof verify 0001 0002), or pass --stale or --all");
        }
        println!("nothing to verify: every receipt is current");
        return Ok(0);
    }
    let outcomes = verify_specs(&project, &selected, refresh, &mut Session::default())?;
    print_outcomes(&outcomes);
    let failed = outcomes.iter().filter(|outcome| outcome.failed()).count();
    if failed > 0 {
        println!("{failed} of {} failed; their receipts are unchanged", outcomes.len());
        return Ok(1);
    }
    Ok(0)
}

/// One spec's verify.
pub struct Outcome {
    pub name: String,
    pub result: Verdict,
}

pub enum Verdict {
    /// Every failure mode passed and the receipt was rewritten (it was stale, or `--refresh`).
    Refreshed(Verified),
    /// Every failure mode passed on a receipt that was current; nothing was written.
    Unchanged(Verified),
    /// The spec has no receipt: nothing to re-prove, and not a failure.
    Unrecorded,
    Failed(String),
}

pub struct Verified {
    pub modes: usize,
    /// The footprint in a few words (`footprint 14 files, coverage`), so a fallback to the diff shows.
    pub footprint: String,
    /// The blake3 of receipt.json as it stands after the verify, which names exactly the receipt state proven.
    pub receipt: String,
    /// How long the green run took.
    pub elapsed: Duration,
}

impl Outcome {
    pub fn failed(&self) -> bool {
        matches!(self.result, Verdict::Failed(_))
    }

    /// The receipt hash a passing verify vouches for.
    pub fn receipt(&self) -> Option<&str> {
        match &self.result {
            Verdict::Refreshed(verified) | Verdict::Unchanged(verified) => Some(&verified.receipt),
            Verdict::Unrecorded | Verdict::Failed(_) => None,
        }
    }
}

/// Reruns green for each spec in order, sharing one session so a runner's setup and build run once.
pub fn verify_specs(
    project: &Project,
    specs: &[SpecDir],
    refresh: bool,
    session: &mut Session,
) -> Result<Vec<Outcome>> {
    let root = project.root.as_path();
    let repo = Repo::open(root)?;
    let head = repo.head()?;
    let scratch = tempfile::Builder::new().prefix("skies-proof-").tempdir()?;
    let mut outcomes = Vec::new();
    for spec in specs {
        let context = Context {
            project,
            repo: &repo,
            head: &head,
            refresh,
        };
        let result = match verify_one(&context, spec, session, scratch.path()) {
            Ok(Ok(Some((modes, footprint, wrote, elapsed)))) => {
                let verified = Verified {
                    modes,
                    footprint,
                    elapsed,
                    receipt: hash::hash_file(&spec.file(RECEIPT_FILE)).unwrap_or_else(|| hash::ABSENT.to_string()),
                };
                if wrote {
                    Verdict::Refreshed(verified)
                } else {
                    Verdict::Unchanged(verified)
                }
            }
            Ok(Ok(None)) => Verdict::Unrecorded,
            Ok(Err(finding)) => Verdict::Failed(finding),
            Err(error) => Verdict::Failed(format!("{error:#}")),
        };
        outcomes.push(Outcome {
            name: spec.name.clone(),
            result,
        });
    }
    Ok(outcomes)
}

/// `<spec>  verified  n/n FMs pass`, `verified (current, unchanged)`, `unrecorded`, or `failed` with the reason
/// indented under it.
pub fn print_outcomes(outcomes: &[Outcome]) {
    let width = outcomes.iter().map(|outcome| outcome.name.len()).max().unwrap_or(0);
    for outcome in outcomes {
        let (verdict, detail) = match &outcome.result {
            Verdict::Refreshed(verified) => (
                "verified".to_string(),
                format!(
                    "{0}/{0} FMs pass (refreshed; {1}; {2})",
                    verified.modes,
                    verified.footprint,
                    seconds(verified.elapsed)
                ),
            ),
            Verdict::Unchanged(verified) => (
                "verified (current, unchanged)".to_string(),
                format!("{0}/{0} FMs pass ({1})", verified.modes, seconds(verified.elapsed)),
            ),
            Verdict::Unrecorded => (
                "unrecorded".to_string(),
                format!("no receipt yet; `skies proof record {}` proves it", outcome.name),
            ),
            Verdict::Failed(finding) => ("failed".to_string(), finding.clone()),
        };
        let mut lines = detail.lines();
        println!(
            "{:<width$}  {verdict}  {}",
            outcome.name,
            lines.next().unwrap_or_default()
        );
        for line in lines {
            println!("{:<width$}    {line}", "");
        }
    }
}

/// Named specs, then stale ones, then all, deduplicated, in id order.
fn select(project: &Project, keys: &[String], stale: bool, all: bool) -> Result<Vec<SpecDir>> {
    let root = project.root.as_path();
    let mut chosen: BTreeSet<String> = BTreeSet::new();
    for key in keys {
        chosen.insert(spec::find(root, key)?.name);
    }
    for spec in spec::discover(root)? {
        let pick = all || (stale && matches!(receipt::freshness(project, &spec)?, Freshness::Stale(_)));
        if pick {
            chosen.insert(spec.name);
        }
    }
    Ok(spec::discover(root)?
        .into_iter()
        .filter(|spec| chosen.contains(&spec.name))
        .collect())
}

struct Context<'a> {
    project: &'a Project,
    repo: &'a Repo,
    head: &'a str,
    refresh: bool,
}

/// `Ok(Ok(Some((n, footprint, wrote, elapsed))))` when all n failure modes pass (`wrote` when the receipt was rewritten),
/// `Ok(Ok(None))` for a spec without a receipt, `Ok(Err(why))` when the run does not prove the spec, and `Err` when
/// it could not run at all.
type OneResult = Result<Result<Option<(usize, String, bool, Duration)>, String>>;

fn verify_one(context: &Context, spec: &SpecDir, session: &mut Session, scratch: &Path) -> OneResult {
    let project = context.project;
    let root = project.root.as_path();
    let Some(mut receipt) = Receipt::load(spec)? else {
        return Ok(Ok(None));
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
    let freshness = receipt::freshness(project, spec)?;
    if let (Freshness::Tampered(edited), false) = (&freshness, context.refresh) {
        return Ok(Err(format!(
            "evidence edited since recording ({}); `--refresh` replaces the green evidence (red keeps its recorded \
             hashes), or re-record the spec",
            edited.join(", ")
        )));
    }

    let proven = match green::run_green(root, spec, &doc, project, session, scratch)? {
        GreenOutcome::Proven(proven) => proven,
        GreenOutcome::Refuted(message) => return Ok(Err(message)),
    };
    let count = proven.cases.len();
    let elapsed = proven.elapsed;
    if matches!(freshness, Freshness::Current) && !context.refresh {
        return Ok(Ok(Some((count, describe(&receipt), false, elapsed))));
    }
    green::publish_evidence(
        spec,
        &proven.staged,
        &[(proven.file.clone(), proven.report.clone())],
        true,
        &Scrub::new(&[&context.repo.top]),
    )?;

    refresh_footprint(root, &doc, footprint::runner_of(project, &doc), &mut receipt, &proven)?;
    receipt.runner = doc.runner(spec)?.to_string();
    receipt.green.commit = context.head.to_string();
    receipt.green.dirty = context.repo.dirty()?;
    receipt.green.cases = proven.cases;
    receipt.green.report = proven.report;
    receipt.inputs = hash::hash_all(root, &hash::input_paths(root, spec)?);
    receipt.evidence = Some(green::evidence_hashes(spec, receipt.evidence.as_ref())?);
    receipt.save(spec)?;
    Ok(Ok(Some((count, describe(&receipt), true, elapsed))))
}

/// `footprint 14 files, coverage, 13 by executed lines`.
fn describe(receipt: &Receipt) -> String {
    format!(
        "footprint {}, {}",
        footprint::files(receipt.footprint.len()),
        match receipt.footprint_source {
            Source::Coverage => format!("coverage, {} by executed lines", lines::by_lines(&receipt.footprint)),
            Source::Diff => "diff".to_string(),
        }
    )
}

/// Re-anchors the footprint to today's files. The changed part is never recomputed from git (that would sweep in
/// every commit since red): a coverage receipt keeps its recorded `footprint_changed`, and a diff receipt its
/// recorded files. When this green run wrote coverage, the executed part and its executed lines are replaced by what
/// it executed today, so the footprint follows the code as it evolves, and a diff receipt is upgraded to coverage.
/// Without coverage the recorded file set is kept, each file pinned whole: today's run executed lines this one cannot
/// see. `touches` is re-matched either way, and everything is limited to the runner's scope.
fn refresh_footprint(
    root: &Path,
    doc: &SpecDoc,
    runner: &Runner,
    receipt: &mut Receipt,
    proven: &ProvenGreen,
) -> Result<()> {
    let touched = footprint::touched(root, doc, runner)?;
    let recorded: BTreeSet<String> = receipt.footprint.keys().cloned().collect();
    let (paths, executed) = match (&proven.coverage, receipt.footprint_source) {
        (coverage::Outcome::Covered(_), source) => {
            let changed = match source {
                Source::Coverage => receipt.footprint_changed.clone(),
                Source::Diff => footprint::recorded_changed(&recorded, &touched),
            };
            let refreshed = footprint::build(root, doc, runner, &changed, proven)?;
            receipt.footprint_source = refreshed.source;
            receipt.footprint_changed = refreshed.changed;
            (refreshed.paths, refreshed.executed)
        }
        (coverage::Outcome::Missing(_), _) => {
            let mut paths = recorded;
            paths.extend(touched);
            paths.retain(|path| !hash::is_spec_path(path) && runner.in_scope(path));
            (paths, BTreeMap::new())
        }
    };
    receipt.footprint = lines::print_all(root, &paths, &executed);
    Ok(())
}
