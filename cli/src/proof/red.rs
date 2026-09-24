//! Red: the spec's cases on a revision without the feature, in a throwaway worktree.
//!
//! Every failure mode must fail there. Cases that cannot even run count as failing (`did-not-build`), whatever the
//! runner: no report at all (.NET E2E that reference a type the feature adds), or a report in which no case names a
//! failure mode while something failed (vitest's one file-level case for an import that does not exist yet,
//! Playwright or Flutter failing at load). The runner's output is kept as `evidence/red.log` in that case, and
//! printed whenever red does not match spec.md, so a wrong red revision is visible instead of a riddle.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Result;

use super::git::Repo;
use super::receipt::{Entry, RedCase};
use super::report::{self, Case, FmId, Outcome, fm_ids};
use super::runner::{Job, NoReport, Run, Session, tail};
use super::scrub::Scrub;
use super::spec::{E2E_DIR, EVIDENCE_DIR, RECEIPT_FILE, SPEC_FILE, SpecDir, SpecDoc};
use super::{avp, green};
use crate::manifest::Runner;

/// What red established, ready to be published with green.
pub struct Red {
    pub cases: BTreeMap<FmId, Entry<RedCase>>,
    /// The report or log to publish, and its path inside the spec folder.
    pub file: PathBuf,
    pub report: String,
    /// Where the red run's `{evidence}` went, so Assay verdicts saved there survive the worktree.
    pub evidence: PathBuf,
    /// Built while the worktree existed: red and green paths both become `{root}` in the evidence.
    pub scrub: Scrub,
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
    session: &mut Session,
    scratch: &Path,
) -> Result<Option<Red>> {
    let evidence = scratch.join("red-evidence");
    let (outcome, scrub) = {
        let worktree = repo.temp_worktree(revision.commit)?;
        let scrub = Scrub::new(&[&repo.top, &worktree.path]);
        let red_root = worktree.path.join(&repo.prefix);
        let red_spec = SpecDir {
            path: red_root.join(spec.rel()),
            ..spec.clone()
        };
        copy_spec_sources(spec, &red_spec)?;
        if let Some(patch) = revision.patch {
            repo.apply(&worktree.path, patch)?;
        }
        let run = session.run(&Job {
            runner_name: runner.0,
            runner: runner.1,
            spec: &red_spec,
            root: &red_root,
            evidence: &evidence,
            scratch,
            label: "red",
        });
        // A configured report path lives inside the worktree; keep the report and the log past its removal.
        let log = scratch.join("red.kept.log");
        let outcome = match run {
            Ok(run) => {
                std::fs::copy(&run.log, &log)?;
                let kept = scratch.join("red.report");
                std::fs::copy(&run.file, &kept)?;
                RedRun::Report(run, kept, log)
            }
            Err(error) => match error.downcast_ref::<NoReport>() {
                Some(no_report) => {
                    std::fs::copy(&no_report.log, &log)?;
                    RedRun::NoReport(log)
                }
                None => return Err(error),
            },
        };
        (outcome, scrub)
    };
    let (run, file, log) = match outcome {
        RedRun::NoReport(log) => {
            return Ok(Some(did_not_build(
                doc,
                "the runner wrote no report",
                log,
                evidence,
                scrub,
            )));
        }
        RedRun::Report(run, file, log) => (run, file, log),
    };
    if let Some(why) = never_ran(&run.report.cases, run.success) {
        return Ok(Some(did_not_build(doc, why, log, evidence, scrub)));
    }
    let evaluation = match report::evaluate(&doc.failure_modes, &run.report.cases) {
        Ok(evaluation) => evaluation,
        Err(problems) => {
            mismatch(spec, &problems.to_string(), &log)?;
            return Ok(None);
        }
    };
    let cases = evaluation
        .passed
        .iter()
        .map(|(id, passed)| (*id, entry(doc, *id, *passed, &evidence)))
        .collect();
    Ok(Some(Red {
        cases,
        file,
        report: format!("{EVIDENCE_DIR}/red.{}", run.report.format.extension()),
        evidence,
        scrub,
    }))
}

/// What the red checkout produced: a report (with its kept copy and log), or only a log.
enum RedRun {
    Report(Run, PathBuf, PathBuf),
    NoReport(PathBuf),
}

/// Every failure mode fails red because its cases never ran; the output is the evidence.
fn did_not_build(doc: &SpecDoc, why: &str, log: PathBuf, evidence: PathBuf, scrub: Scrub) -> Red {
    println!("  red did not build: {why}, so every failure mode counts as failing (output in {EVIDENCE_DIR}/red.log)");
    let cases = doc
        .failure_modes
        .iter()
        .map(|id| (*id, Entry::new(RedCase::DidNotBuild, doc.avp(*id), None)))
        .collect();
    Red {
        cases,
        file: log,
        report: format!("{EVIDENCE_DIR}/red.log"),
        evidence,
        scrub,
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

/// Says why red does not match spec.md, with the runner's last output, and keeps that output as the spec's
/// `evidence/red.log` when the spec has no receipt whose evidence it would overwrite.
fn mismatch(spec: &SpecDir, problems: &str, log: &Path) -> Result<()> {
    eprintln!("{}: the red run does not match spec.md:\n{problems}", spec.name);
    eprintln!("red's last output:\n{}", tail(log));
    if !spec.file(RECEIPT_FILE).is_file() {
        let evidence = spec.file(EVIDENCE_DIR);
        std::fs::create_dir_all(&evidence)?;
        std::fs::copy(log, evidence.join("red.log"))?;
        eprintln!("(the whole output is in {}/{EVIDENCE_DIR}/red.log)", spec.rel());
    }
    eprintln!(
        "If red is the wrong revision (see the `red` line above), pass --red <rev> or set `default_branch` under \
         [workspace] in Skies.toml."
    );
    Ok(())
}

/// A failure mode passes red, and so bites nothing, only as green would count it passing: every case passed and,
/// for a tagged mode, the verdict passed too. A missing verdict on red is simply a failure.
fn entry(doc: &SpecDoc, id: FmId, cases_passed: bool, evidence: &Path) -> Entry<RedCase> {
    let result = if cases_passed && avp::check(doc, id, evidence).is_ok() {
        RedCase::NonDiscriminating
    } else {
        RedCase::Fail
    };
    let name = avp::verdict_file(id);
    let verdict = evidence
        .join(&name)
        .is_file()
        .then(|| format!("{EVIDENCE_DIR}/red.{name}"));
    Entry::new(result, doc.avp(id), verdict)
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
