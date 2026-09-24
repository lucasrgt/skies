//! Green: running a spec on the working tree and judging each failure mode.
//!
//! Shared by `record` (after red) and `run` (judged and printed, nothing committed). A failure mode passes green when
//! every case naming it passes and, if spec.md tags it `[avp: …]`, the Assay verdict its case saved reports every
//! tagged criterion as passing. Artifacts are staged outside the spec folder until the run is known to be good, so a
//! failing run never overwrites the evidence of the last good one.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Result;

use super::avp;
use super::report::{self, Case, FmId, Inconsistency};
use super::runner::{self, Job, Run, tail};
use super::spec::{SpecDir, SpecDoc};
use super::summary;
use crate::manifest::Project;

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
    /// Where the runner's artifacts (`$SKIES_EVIDENCE`) were staged.
    pub staged: PathBuf,
    /// Every failure mode's result, or why the run's cases and spec.md do not agree at all.
    pub modes: Result<BTreeMap<FmId, Mode>, Inconsistency>,
    /// The cases naming each failure mode (empty when the run and spec.md disagree).
    pub cases: BTreeMap<FmId, Vec<Case>>,
}

impl Checked {
    /// The runner's last lines of output, for a message.
    pub fn output(&self) -> String {
        format!("last output:\n{}", tail(&self.run.log))
    }

    /// The sorted names of the cases naming `id`.
    pub fn names(&self, id: FmId) -> Vec<String> {
        summary::names(self.cases.get(&id).map(Vec::as_slice).unwrap_or_default())
    }
}

/// Runs the spec on the working tree and judges each failure mode.
pub fn check(project: &Project, spec: &SpecDir, doc: &SpecDoc, scratch: &Path) -> Result<Checked> {
    let runner_name = doc.runner(spec)?;
    let staged = scratch.join("green-evidence");
    let run = runner::run(&Job {
        runner_name,
        runner: project.runner(runner_name)?,
        spec,
        root: &project.root,
        evidence: &staged,
        scratch,
        label: "green",
    })?;
    let evaluation = report::evaluate(&doc.failure_modes, &run.report.cases);
    let cases = evaluation
        .as_ref()
        .map(|evaluation| evaluation.cases.clone())
        .unwrap_or_default();
    let modes = evaluation.map(|evaluation| {
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
    Ok(Checked {
        run,
        staged,
        modes,
        cases,
    })
}

/// Why a green run does not prove the spec, or `None` when every failure mode passes.
pub fn refutation(checked: &Checked) -> Option<String> {
    let modes = match &checked.modes {
        Ok(modes) => modes,
        Err(problems) => {
            return Some(format!(
                "the green run does not match spec.md:\n{problems}\n{}",
                checked.output()
            ));
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
    if !failing.is_empty() {
        problems.push(checked.output());
    }
    (!problems.is_empty()).then(|| problems.join("\n"))
}

pub fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

pub fn join(ids: &[FmId]) -> String {
    ids.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")
}
