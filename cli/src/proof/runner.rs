//! Running a spec's E2E through the runner declared in `Skies.toml`.
//!
//! A runner is just a shell command. The engine fills in placeholders (quoted for their spot, see `shell`), runs it
//! with the checkout as the working directory, and reads back the report; it never interprets the command's exit
//! code, because on the red revision failing tests are the expected outcome. Only a missing or unreadable report is
//! an error.

use std::collections::BTreeMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use super::report::{self, Report};
use super::shell::{self, expand};
use super::spec::{E2E_DIR, SpecDir};
use crate::manifest::Runner;

/// One run of one spec in one checkout.
pub struct Job<'a> {
    pub runner_name: &'a str,
    pub runner: &'a Runner,
    pub spec: &'a SpecDir,
    /// The project root inside the checkout being exercised (the working tree, or the red worktree).
    pub root: &'a Path,
    /// Where the runner may drop artifacts (`{evidence}`, `$SKIES_EVIDENCE`).
    pub evidence: &'a Path,
    /// A scratch directory for the report and the captured log.
    pub scratch: &'a Path,
    /// `red` or `green`, for messages and file names.
    pub label: &'a str,
}

/// The runner finished without writing a report: the tests did not build or did not start. On red that is the
/// expected state of a feature whose E2E references code that does not exist yet.
#[derive(Debug)]
pub struct NoReport {
    pub message: String,
    /// The runner's captured output.
    pub log: PathBuf,
}

impl std::fmt::Display for NoReport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for NoReport {}

/// A finished run: the parsed report and its file, the runner's captured output, and its exit status.
pub struct Run {
    pub report: Report,
    pub file: PathBuf,
    pub log: PathBuf,
    /// Whether the command exited 0. Never decides a failure mode (the report does), but a red run whose report
    /// names no failure mode and whose command failed did not get far enough to run the cases.
    pub success: bool,
    pub elapsed: Duration,
}

/// Runs one spec in one checkout: `setup`, then `build`, then `command`, each once. Every run gets fresh scratch
/// paths, so nothing an earlier run left can count as this run's report or evidence.
pub fn run(job: &Job) -> Result<Run> {
    let started = Instant::now();
    let values = placeholders(job)?;
    // Tests find where to drop artifacts (an Assay verdict, a screenshot) and which spec they serve, whatever the test
    // framework and however the command is written.
    let env = BTreeMap::from([
        ("SKIES_EVIDENCE", values["evidence"].clone()),
        ("SKIES_SPEC", values["spec"].clone()),
    ]);
    std::fs::create_dir_all(job.evidence)?;
    if let Some(setup) = &job.runner.setup {
        let log = job.scratch.join(format!("{}-setup.log", job.label));
        if !execute(&expand(setup, &values), job.root, &env, &log)? {
            bail!(
                "runner '{}' setup failed on {}:\n{}",
                job.runner_name,
                job.label,
                tail(&log)
            );
        }
    }
    let log = job.scratch.join(format!("{}.log", job.label));
    if let Some(build) = &job.runner.build
        && !execute(&expand(build, &values), job.root, &env, &log)?
    {
        return Err(no_report(job, "build failed", log));
    }
    let success = execute(&expand(&job.runner.command, &values), job.root, &env, &log)?;
    let report_path = PathBuf::from(&values["report"]);
    let Ok(text) = std::fs::read_to_string(&report_path) else {
        return Err(no_report(job, "wrote no report (did the tests build?)", log));
    };
    let report =
        report::parse(&text).with_context(|| format!("reading the {} report {}", job.label, report_path.display()))?;
    Ok(Run {
        report,
        file: report_path,
        log,
        success,
        elapsed: started.elapsed(),
    })
}

fn no_report(job: &Job, what: &str, log: PathBuf) -> anyhow::Error {
    NoReport {
        message: format!(
            "runner '{}' {what} on {}. Last output:\n{}",
            job.runner_name,
            job.label,
            tail(&log)
        ),
        log,
    }
    .into()
}

fn placeholders(job: &Job) -> Result<BTreeMap<&'static str, String>> {
    let text = |path: &Path| {
        path.to_str()
            .map(String::from)
            .with_context(|| format!("{} is not valid UTF-8", path.display()))
    };
    Ok(BTreeMap::from([
        ("id", job.spec.id.clone()),
        ("spec", job.spec.name.clone()),
        ("dir", format!("{}/{E2E_DIR}", job.spec.rel())),
        ("evidence", text(job.evidence)?),
        ("report", text(&job.scratch.join(format!("{}-report.xml", job.label)))?),
    ]))
}

/// Runs `command` through `sh -c` (see [`shell`](super::shell)), output captured to `log`. Returns whether it
/// exited successfully.
fn execute(command: &str, cwd: &Path, env: &BTreeMap<&str, String>, log: &Path) -> Result<bool> {
    let out = File::create(log).with_context(|| format!("creating {}", log.display()))?;
    let err = out.try_clone()?;
    let status = Command::new(shell::program()?)
        .arg("-c")
        .arg(command)
        .current_dir(cwd)
        .envs(env)
        .stdin(Stdio::null())
        .stdout(out)
        .stderr(err)
        .status()
        .with_context(|| format!("starting `{command}`"))?;
    Ok(status.success())
}

/// `12.3 s`, for the line that reports a run.
pub fn seconds(elapsed: Duration) -> String {
    format!("{:.1} s", elapsed.as_secs_f64())
}

/// The last lines of a log, enough to see a build error without flooding the terminal.
pub fn tail(log: &Path) -> String {
    const LINES: usize = 30;
    let text = std::fs::read_to_string(log).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(LINES)..]
        .iter()
        .map(|line| format!("  | {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}
