//! Green: running a spec on the working tree, and publishing what it proved into the spec's evidence/.
//!
//! Shared by `record` (after red), `verify` (green alone), and `run` (green, judged and printed, nothing kept). A
//! failure mode passes green when every case naming it passes and, if spec.md tags it `[avp: …]`, the Assay verdict
//! its case saved reports every tagged criterion as passing. Evidence is staged outside the spec folder until the
//! run is known to be good, so a failing run never overwrites the evidence of the last good one.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};

use super::hash::{self, Hashes};
use super::receipt::{Entry, GreenCase};
use super::report::{self, FmId, Inconsistency};
use super::runner::{Job, Run, Session, tail};
use super::scrub::Scrub;
use super::spec::{EVIDENCE_DIR, SpecDir, SpecDoc};
use super::{avp, coverage};
use crate::manifest::Project;

pub struct ProvenGreen {
    pub cases: BTreeMap<FmId, Entry<GreenCase>>,
    /// The report file name inside evidence/, relative to the spec folder.
    pub report: String,
    /// The report as the runner wrote it.
    pub file: PathBuf,
    /// Artifacts the runner wrote to `{evidence}` / `$SKIES_EVIDENCE`, staged until the run is known to be good.
    pub staged: PathBuf,
    /// The project files the run executed, or why there is no telling. Coverage never enters evidence: the
    /// footprint hashes it produces are the record.
    pub coverage: coverage::Outcome,
    /// The coverage location as a project path when the runner writes it inside the project, so the report itself
    /// never counts as a changed file.
    pub coverage_artifact: Option<String>,
    pub elapsed: Duration,
}

pub enum GreenOutcome {
    Proven(ProvenGreen),
    /// The run finished but does not prove the spec; the message says why, with the run's last output.
    Refuted(String),
}

/// How one failure mode fared in a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Pass,
    /// A case naming it failed or was skipped.
    CasesFailed,
    /// Its cases passed but its Assay verdict does not prove it; the message says why.
    Unproven(String),
}

/// A spec run on the working tree, judged mode by mode but not yet accepted.
pub struct Checked {
    pub run: Run,
    pub staged: PathBuf,
    /// Every failure mode's result, or why the run's cases and spec.md do not agree at all.
    pub modes: Result<BTreeMap<FmId, Mode>, Inconsistency>,
}

impl Checked {
    /// The runner's last lines of output, indented, for a message.
    pub fn output(&self) -> String {
        format!("last output:\n{}", tail(&self.run.log))
    }
}

/// Runs the spec on the working tree and judges each failure mode.
pub fn check(
    root: &Path,
    spec: &SpecDir,
    doc: &SpecDoc,
    project: &Project,
    session: &mut Session,
    scratch: &Path,
) -> Result<Checked> {
    let runner_name = doc.runner(spec)?;
    let staged = scratch.join(format!("{}-evidence", spec.name));
    let run = session.run(&Job {
        runner_name,
        runner: project.runner(runner_name)?,
        spec,
        root,
        evidence: &staged,
        scratch,
        label: "green",
    })?;
    let modes = report::evaluate(&doc.failure_modes, &run.report.cases).map(|evaluation| {
        evaluation
            .passed
            .iter()
            .map(|(id, passed)| {
                // A verdict is only worth reading for a mode whose cases passed; a failing case already says enough.
                let mode = match (*passed, avp::check(doc, *id, &staged)) {
                    (false, _) => Mode::CasesFailed,
                    (true, Ok(())) => Mode::Pass,
                    (true, Err(why)) => Mode::Unproven(why),
                };
                (*id, mode)
            })
            .collect()
    });
    Ok(Checked { run, staged, modes })
}

/// Runs the spec on the working tree and decides whether it proves every failure mode.
pub fn run_green(
    root: &Path,
    spec: &SpecDir,
    doc: &SpecDoc,
    project: &Project,
    session: &mut Session,
    scratch: &Path,
) -> Result<GreenOutcome> {
    let checked = check(root, spec, doc, project, session, scratch)?;
    let modes = match &checked.modes {
        Ok(modes) => modes,
        Err(problems) => {
            return Ok(GreenOutcome::Refuted(format!(
                "the green run does not match spec.md:\n{problems}\n{}",
                checked.output()
            )));
        }
    };
    let failing: Vec<FmId> = modes
        .iter()
        .filter(|(_, mode)| **mode == Mode::CasesFailed)
        .map(|(id, _)| *id)
        .collect();
    let mut problems = Vec::new();
    if !failing.is_empty() {
        problems.push(format!(
            "{} not passing on the working tree (a failed or skipped case counts as not passing)",
            join(&failing)
        ));
    }
    problems.extend(modes.values().filter_map(|mode| match mode {
        Mode::Unproven(why) => Some(why.clone()),
        _ => None,
    }));
    if !problems.is_empty() {
        if !failing.is_empty() {
            problems.push(checked.output());
        }
        return Ok(GreenOutcome::Refuted(problems.join("\n")));
    }
    let cases = modes
        .keys()
        .map(|id| {
            let verdict = format!("{EVIDENCE_DIR}/{}", avp::verdict_file(*id));
            (*id, Entry::new(GreenCase::Pass, doc.avp(*id), Some(verdict)))
        })
        .collect();
    let Checked { run, staged, .. } = checked;
    let runner_name = doc.runner(spec)?;
    // An unreadable coverage file costs the footprint its precision, never the proof.
    let covered = coverage::collect(&run.coverage, runner_name, root, &[root])
        .unwrap_or_else(|error| coverage::Outcome::Missing(format!("could not read coverage: {error:#}")));
    let coverage_artifact = run
        .coverage
        .path
        .starts_with(root)
        .then(|| hash::relative(root, &run.coverage.path));
    Ok(GreenOutcome::Proven(ProvenGreen {
        cases,
        report: format!("{EVIDENCE_DIR}/green.{}", run.report.format.extension()),
        file: run.file,
        staged,
        coverage: covered,
        coverage_artifact,
        elapsed: run.elapsed,
    }))
}

/// Replaces evidence/ with the staged runner artifacts plus the given files (`(source, path in spec folder)`), the
/// reports and logs among them scrubbed of machine-specific paths and names. With `keep_red`, the red files of the
/// original recording survive, since `verify` never reruns red.
pub fn publish_evidence(
    spec: &SpecDir,
    staged: &Path,
    files: &[(PathBuf, String)],
    keep_red: bool,
    scrub: &Scrub,
) -> Result<()> {
    let evidence = spec.file(EVIDENCE_DIR);
    let mut kept: Vec<(String, Vec<u8>)> = Vec::new();
    if keep_red && evidence.is_dir() {
        for entry in std::fs::read_dir(&evidence)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with("red.") && entry.path().is_file() {
                kept.push((name, std::fs::read(entry.path())?));
            }
        }
    }
    if evidence.exists() {
        std::fs::remove_dir_all(&evidence).with_context(|| format!("clearing {}", evidence.display()))?;
    }
    std::fs::create_dir_all(&evidence)?;
    if staged.is_dir() {
        copy_dir(staged, &evidence)?;
    }
    for (name, bytes) in kept {
        std::fs::write(evidence.join(name), bytes)?;
    }
    for (source, target) in files {
        scrub
            .copy(source, &spec.path.join(target))
            .with_context(|| format!("copying {target} into the spec"))?;
    }
    Ok(())
}

/// Hashes evidence/ as it is now. Red files keep the hash recorded with red (`previous`), because `verify` carries
/// them over without rerunning red: rehashing would launder an edit made to them since.
pub fn evidence_hashes(spec: &SpecDir, previous: Option<&Hashes>) -> Result<Hashes> {
    let mut hashes = hash::evidence(spec)?;
    if let Some(previous) = previous {
        let red = format!("{EVIDENCE_DIR}/red.");
        for (path, recorded) in previous.iter().filter(|(path, _)| path.starts_with(&red)) {
            hashes.insert(path.clone(), recorded.clone());
        }
    }
    Ok(hashes)
}

pub fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from).with_context(|| format!("reading {}", from.display()))? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target).with_context(|| format!("copying {}", entry.path().display()))?;
        }
    }
    Ok(())
}

pub fn join(ids: &[FmId]) -> String {
    ids.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")
}
