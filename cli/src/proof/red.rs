//! Red: the spec's cases on a revision without the feature, in a throwaway worktree.
//!
//! Every failure mode must fail there. Cases that cannot even run count as failing (`did-not-build`), whatever the
//! runner: no report at all (.NET E2E that reference a type the feature adds), or a report in which no case names a
//! failure mode while something failed (vitest's one file-level case for an import that does not exist yet,
//! Playwright or Flutter failing at load). The runner's output is kept locally as `evidence/raw/red.log` and printed
//! whenever red does not match spec.md, so a wrong red revision is visible instead of a riddle.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::evidence::RAW_DIR;
use super::git::Repo;
use super::receipt::RedResult;
use super::report::{self, Case, FmId, Outcome, fm_ids};
use super::runner::{self, Job, NoReport, tail};
use super::spec::{E2E_DIR, EVIDENCE_DIR, SPEC_FILE, SpecDir, SpecDoc};
use super::summary::{self, Scrub};
use super::{avp, green};
use crate::manifest::Runner;

/// What red established.
pub struct Red {
    pub results: BTreeMap<FmId, RedResult>,
    /// Per mode, the start of what its first failing case said.
    pub messages: BTreeMap<FmId, String>,
    /// The runner's report and its extension; `None` when red did not build.
    pub report: Option<(PathBuf, &'static str)>,
    /// The runner's output.
    pub log: PathBuf,
    /// When red did not build, the lines of output that say why.
    pub output: Option<String>,
    /// Where the red run's `{evidence}` went (Assay verdicts).
    pub evidence: PathBuf,
}

impl Red {
    /// The verdict red saved for a tagged mode, if its case got far enough to write one.
    pub fn verdict(&self, doc: &SpecDoc, id: FmId) -> Option<PathBuf> {
        let file = self.evidence.join(avp::verdict_file(id));
        (!doc.avp(id).is_empty() && file.is_file()).then_some(file)
    }
}

/// Where red runs: a commit, optionally with a patch that removes the feature.
pub struct Revision<'a> {
    pub commit: &'a str,
    pub patch: Option<&'a Path>,
}

/// Runs red and reads it. `Ok(None)` when red does not match spec.md, after saying why on stderr.
pub fn run(
    repo: &Repo,
    spec: &SpecDir,
    doc: &SpecDoc,
    runner: (&str, &Runner),
    revision: &Revision,
    scratch: &Path,
) -> Result<Option<Red>> {
    // The report, the log, and the evidence land in the scratch folder, so they outlive the worktree.
    let evidence = scratch.join("red-evidence");
    let worktree = repo.temp_worktree(revision.commit)?;
    let scrub = Scrub::new(&[&repo.top, &worktree.path]);
    let red_root = worktree.path.join(&repo.prefix);
    let red_spec = SpecDir {
        path: red_root.join(spec.rel()),
        ..spec.clone()
    };
    copy_spec_sources(spec, &red_spec)?;
    if let Some(patch) = revision.patch {
        repo.apply(&worktree.path, patch).with_context(|| {
            format!(
                "{} no longer removes the feature from {}: stub the feature out again, save the diff \
                 (`git diff --relative > {}/red.patch`), restore the code, and record again",
                patch.display(),
                &revision.commit[..revision.commit.len().min(7)],
                spec.rel(),
            )
        })?;
    }
    let job = Job {
        runner_name: runner.0,
        runner: runner.1,
        spec: &red_spec,
        root: &red_root,
        evidence: &evidence,
        scratch,
        label: "red",
    };
    let run = match runner::run(&job) {
        Ok(run) => run,
        Err(error) => match error.downcast_ref::<NoReport>() {
            Some(no_report) => {
                let log = no_report.log.clone();
                let why = "the runner wrote no report";
                return Ok(Some(did_not_build(doc, why, log, evidence, &scrub)));
            }
            None => return Err(error),
        },
    };
    if let Some(why) = never_ran(&run.report.cases, run.success) {
        return Ok(Some(did_not_build(doc, why, run.log, evidence, &scrub)));
    }
    let evaluation = match report::evaluate(&doc.failure_modes, &run.report.cases) {
        Ok(evaluation) => evaluation,
        Err(problems) => {
            mismatch(spec, &problems.to_string(), &run.log)?;
            return Ok(None);
        }
    };
    let mut messages = BTreeMap::new();
    let mut results = BTreeMap::new();
    for (id, passed) in &evaluation.passed {
        let named = evaluation.cases.get(id).map(Vec::as_slice).unwrap_or_default();
        if let Some(message) = summary::first_failure(named, &scrub) {
            messages.insert(*id, message);
        }
        // A mode passes red, and so bites nothing, only as green would count it passing: every case passed and, for
        // a tagged mode, the verdict passed too. A missing verdict on red is simply a failure.
        let bites = !*passed || avp::check(doc, *id, &evidence).is_err();
        let result = if bites {
            RedResult::Fail
        } else {
            RedResult::NonDiscriminating
        };
        results.insert(*id, result);
    }
    Ok(Some(Red {
        results,
        messages,
        report: Some((run.file, run.report.format.extension())),
        log: run.log,
        output: None,
        evidence,
    }))
}

/// Every failure mode fails red because its cases never ran; the output says why.
fn did_not_build(doc: &SpecDoc, why: &str, log: PathBuf, evidence: PathBuf, scrub: &Scrub) -> Red {
    println!(
        "  red did not build: {why}, so every failure mode counts as failing (output in {EVIDENCE_DIR}/{RAW_DIR}/red.log)"
    );
    Red {
        results: doc
            .failure_modes
            .iter()
            .map(|id| (*id, RedResult::DidNotBuild))
            .collect(),
        messages: BTreeMap::new(),
        report: None,
        output: summary::output(&log, scrub),
        log,
        evidence,
    }
}

/// Why a red report shows the cases never ran, if it does: no case names a failure mode, and either one failed or
/// errored (a file that could not load) or the runner exited non-zero. Only ever asked of red; on green the same
/// report is a mismatch with spec.md.
pub fn never_ran(cases: &[Case], success: bool) -> Option<&'static str> {
    if cases.iter().any(|case| !fm_ids(&case.name).is_empty()) {
        return None;
    }
    if cases.iter().any(|case| case.outcome == Outcome::Failed) {
        Some("no case names a failure mode and the report has a failed or errored case")
    } else if !success {
        Some("no case names a failure mode and the runner exited non-zero")
    } else {
        None
    }
}

/// Says why red does not match spec.md, with the runner's last output, and keeps that output locally as the spec's
/// `evidence/raw/red.log`.
fn mismatch(spec: &SpecDir, problems: &str, log: &Path) -> Result<()> {
    eprintln!("{}: the red run does not match spec.md:\n{problems}", spec.name);
    eprintln!("red's last output:\n{}", tail(log));
    let raw = spec.file(EVIDENCE_DIR).join(RAW_DIR);
    std::fs::create_dir_all(&raw)?;
    std::fs::copy(log, raw.join("red.log"))?;
    eprintln!(
        "(the whole output is in {}/{EVIDENCE_DIR}/{RAW_DIR}/red.log)",
        spec.rel()
    );
    eprintln!(
        "If red is the wrong revision (see the `red` line above), pass --red <rev> or set `default_branch` under \
         [workspace] in Skies.toml."
    );
    Ok(())
}

/// The spec usually does not exist at the red revision, so its spec.md and e2e/ come from the working tree.
fn copy_spec_sources(from: &SpecDir, to: &SpecDir) -> Result<()> {
    let e2e = to.file(E2E_DIR);
    if e2e.exists() {
        std::fs::remove_dir_all(&e2e)?;
    }
    std::fs::create_dir_all(&to.path)?;
    std::fs::copy(from.file(SPEC_FILE), to.file(SPEC_FILE))?;
    if from.file(E2E_DIR).is_dir() {
        green::copy_dir(&from.file(E2E_DIR), &e2e)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case(name: &str, outcome: Outcome) -> Case {
        Case {
            name: name.into(),
            outcome,
            message: None,
        }
    }

    #[test]
    fn cases_that_never_ran_are_told_apart_from_a_mismatch() {
        let load_error = [case("frontend/web/.specs/0010/e2e/transfer.test.tsx", Outcome::Failed)];
        assert!(never_ran(&load_error, false).is_some(), "vitest's file-level failure");
        assert!(never_ran(&load_error, true).is_some());
        assert!(never_ran(&[], false).is_some(), "no cases and a failing runner");
        assert!(never_ran(&[case("helper", Outcome::Passed)], false).is_some());

        assert!(
            never_ran(&[], true).is_none(),
            "an empty, clean run is a mismatch, not a build failure"
        );
        assert!(never_ran(&[case("helper", Outcome::Skipped)], true).is_none());
        let named = [case("FM-1: a", Outcome::Failed), case("setup", Outcome::Failed)];
        assert!(never_ran(&named, false).is_none(), "a case naming a mode ran");
        assert!(never_ran(&[case("FM-9: stray", Outcome::Passed)], false).is_none());
    }
}
