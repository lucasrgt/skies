//! Red: the spec's cases on a revision without the feature, in a throwaway worktree.
//!
//! Every failure mode must fail there. When the cases never ran (no report, or a report in which no case names a
//! failure mode while something failed), every mode counts as failing (`did-not-build`) only if `red_cause` traces
//! the failure to the spec's own e2e files: the E2E reference code the feature adds. Any other reason red could not
//! run its cases (a broken restore, a missing tool, a bad runner command) stops `record` with the output, because
//! calling that red would prove nothing. The runner's output is kept locally as `evidence/raw/red.log` and printed
//! whenever red does not match spec.md, so a wrong red revision is visible instead of a riddle.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use super::evidence::RAW_DIR;
use super::git::Repo;
use super::receipt::RedResult;
use super::red_cause::{self, SpecFiles};
use super::report::{self, Case, FmId, Named, Outcome, case_mode};
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
    /// The runner's report and its extension; `None` when it wrote none.
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
                let never = NeverRan {
                    why: "the runner wrote no report",
                    log: no_report.log.clone(),
                    cases: None,
                    report: None,
                };
                return did_not_build(spec, doc, never, evidence, &scrub).map(Some);
            }
            None => return Err(error),
        },
    };
    if let Some(why) = never_ran(&run.report.cases, run.success) {
        let never = NeverRan {
            why,
            log: run.log,
            cases: Some(&run.report.cases),
            report: Some((run.file.clone(), run.report.format.extension())),
        };
        return did_not_build(spec, doc, never, evidence, &scrub).map(Some);
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

/// A red run whose cases never ran: why it looks so, its output, and its report's cases when it wrote one.
struct NeverRan<'a> {
    why: &'static str,
    log: PathBuf,
    cases: Option<&'a [Case]>,
    /// The report and its extension, kept locally beside the output.
    report: Option<(PathBuf, &'static str)>,
}

/// Every failure mode fails red because its cases never ran, when the spec's own e2e is why; otherwise an error that
/// stops `record` with red's output, and no receipt.
fn did_not_build(spec: &SpecDir, doc: &SpecDoc, never: NeverRan, evidence: PathBuf, scrub: &Scrub) -> Result<Red> {
    let text = std::fs::read_to_string(&never.log).unwrap_or_default();
    let lines = match red_cause::attribute(&text, never.cases, &SpecFiles::new(spec)) {
        Ok(lines) => lines,
        Err(cause) => {
            keep_log(spec, &never.log)?;
            if let Some((file, extension)) = &never.report {
                let raw = spec.file(EVIDENCE_DIR).join(RAW_DIR);
                std::fs::copy(file, raw.join(format!("red.{extension}")))?;
            }
            bail!(
                "red's cases never ran ({}), and not because of the spec's own e2e: {cause}.\n\
                 That red proves nothing, so no receipt was written. Make red build up to the spec's cases (a restore \
                 or tool the red checkout lacks: add a `setup` to the runner; a runner command that fails on its \
                 own: fix it in Skies.toml), or pass --red <rev>. The whole output is in {}/{EVIDENCE_DIR}/{RAW_DIR}/\
                 red.log; its end:\n{}",
                never.why,
                spec.rel(),
                tail(&never.log)
            );
        }
    };
    println!(
        "  red did not build because of the spec's own e2e ({}), so every failure mode counts as failing (output in \
         {EVIDENCE_DIR}/{RAW_DIR}/red.log)",
        summary::excerpt(&lines[..1], scrub).unwrap_or_default()
    );
    Ok(Red {
        results: doc
            .failure_modes
            .iter()
            .map(|id| (*id, RedResult::DidNotBuild))
            .collect(),
        messages: BTreeMap::new(),
        report: never.report,
        output: summary::excerpt(&lines, scrub),
        log: never.log,
        evidence,
    })
}

/// Why a red report shows the cases never ran, if it does: no case names a failure mode (or starts like one), and
/// either one failed or errored (a file that could not load) or the runner exited non-zero. Only ever asked of red;
/// on green the same report is a mismatch with spec.md.
pub fn never_ran(cases: &[Case], success: bool) -> Option<&'static str> {
    if cases.iter().any(|case| case_mode(&case.name) != Named::Nothing) {
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
    keep_log(spec, log)?;
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

/// Keeps red's output as the spec's local `evidence/raw/red.log` when red stops `record`, for a closer look.
fn keep_log(spec: &SpecDir, log: &Path) -> Result<()> {
    let raw = spec.file(EVIDENCE_DIR).join(RAW_DIR);
    std::fs::create_dir_all(&raw)?;
    std::fs::copy(log, raw.join("red.log"))?;
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
            file: None,
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
        assert!(
            never_ran(&[case("FM 1: spaced", Outcome::Failed)], false).is_none(),
            "a look-alike is the grammar's error, not a build failure"
        );
    }
}
