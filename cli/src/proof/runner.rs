//! Running a spec's E2E through the runner declared in `Skies.toml`.
//!
//! A runner is just a shell command. The engine fills in placeholders, runs it with the checkout as the working
//! directory, and reads back the report; it never interprets the command's exit code, because on the red revision
//! failing tests are the expected outcome. Only a missing or unreadable report is an error.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

use super::coverage::Location;
use super::report::{self, Report};
use super::spec::{E2E_DIR, SpecDir};
use crate::manifest::Runner;

/// One run of one spec in one checkout.
pub struct Job<'a> {
    pub runner_name: &'a str,
    pub runner: &'a Runner,
    pub spec: &'a SpecDir,
    /// The project root inside the checkout being exercised (the working tree, or the red worktree).
    pub root: &'a Path,
    /// Where the runner may drop artifacts (`{evidence}`).
    pub evidence: &'a Path,
    /// A scratch directory for the report and the captured log.
    pub scratch: &'a Path,
    /// `red` or `green`, for messages.
    pub label: &'a str,
}

/// The runner finished without writing a report, which means the tests did not build or did not start. On the
/// red revision that is the expected state of a feature whose E2E references code that does not exist yet.
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

/// A finished run: the parsed report and the file it came from, which the caller copies into evidence verbatim, and
/// where the run's coverage landed, if it wrote any.
pub struct Run {
    pub report: Report,
    pub file: PathBuf,
    pub coverage: Location,
}

/// Remembers which setups already ran, so `setup` runs once per runner and checkout in a single invocation even
/// when `verify` exercises many specs.
#[derive(Default)]
pub struct Session {
    done: BTreeSet<(String, PathBuf)>,
}

impl Session {
    pub fn run(&mut self, job: &Job) -> Result<Run> {
        let values = placeholders(job)?;
        let mut env = automatic_env(&values);
        env.extend(
            job.runner
                .env
                .iter()
                .map(|(key, value)| (key.clone(), expand(value, &values))),
        );

        if let Some(setup) = &job.runner.setup
            && self.done.insert((job.runner_name.to_string(), job.root.to_path_buf()))
        {
            let log = job.scratch.join(format!("{}-setup.log", job.label));
            let status = shell(&expand(setup, &values), job.root, &env, &log)?;
            if !status {
                bail!(
                    "runner '{}' setup failed on {}:\n{}",
                    job.runner_name,
                    job.label,
                    tail(&log)
                );
            }
        }

        let report_path = PathBuf::from(&values["report"]);
        if report_path.exists() {
            std::fs::remove_file(&report_path)
                .with_context(|| format!("removing the previous report {}", report_path.display()))?;
        }
        if let Some(parent) = report_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Coverage from an earlier run describes other code; a runner that writes none must find nothing there.
        let coverage = Location {
            path: PathBuf::from(&values["coverage"]),
            configured: job.runner.coverage.is_some() || mentions_coverage(job.runner),
        };
        remove(&coverage.path)?;
        // A verdict or artifact left by an earlier run must never count as this run's evidence.
        if job.evidence.exists() {
            std::fs::remove_dir_all(job.evidence).with_context(|| format!("clearing {}", job.evidence.display()))?;
        }
        std::fs::create_dir_all(job.evidence)?;

        let log = job.scratch.join(format!("{}.log", job.label));
        shell(&expand(&job.runner.command, &values), job.root, &env, &log)?;
        let Ok(text) = std::fs::read_to_string(&report_path) else {
            return Err(NoReport {
                message: format!(
                    "runner '{}' wrote no report at {} on {} (did the tests build?). Last output:\n{}",
                    job.runner_name,
                    report_path.display(),
                    job.label,
                    tail(&log)
                ),
                log,
            }
            .into());
        };
        let report = report::parse(&text)
            .with_context(|| format!("reading the {} report {}", job.label, report_path.display()))?;
        Ok(Run {
            report,
            file: report_path,
            coverage,
        })
    }
}

fn placeholders(job: &Job) -> Result<BTreeMap<&'static str, String>> {
    let mut values = BTreeMap::from([
        ("id", job.spec.id.clone()),
        ("spec", job.spec.name.clone()),
        ("dir", format!("{}/{E2E_DIR}", job.spec.rel())),
        ("evidence", path_text(job.evidence)?),
    ]);
    let report = match &job.runner.report {
        Some(configured) => job.root.join(expand(configured, &values)),
        None => job.scratch.join(format!("{}-report.xml", job.label)),
    };
    values.insert("report", path_text(&report)?);
    let coverage = match &job.runner.coverage {
        Some(configured) => job.root.join(expand(configured, &values)),
        None => job.scratch.join(format!("{}-coverage", job.label)),
    };
    values.insert("coverage", path_text(&coverage)?);
    Ok(values)
}

/// Whether the runner passes `{coverage}` anywhere, which is how a runner without a fixed `coverage` path opts in.
fn mentions_coverage(runner: &Runner) -> bool {
    let placeholder = "{coverage}";
    runner.command.contains(placeholder)
        || runner.setup.as_deref().is_some_and(|setup| setup.contains(placeholder))
        || runner.env.values().any(|value| value.contains(placeholder))
}

/// Deletes a file or a folder, if there is one.
fn remove(path: &Path) -> Result<()> {
    let result = if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else if path.exists() {
        std::fs::remove_file(path)
    } else {
        return Ok(());
    };
    result.with_context(|| format!("removing the previous coverage {}", path.display()))
}

/// Environment every runner gets without configuring it: tests find where to drop artifacts (an Assay verdict, a
/// screenshot), which spec they serve, and where coverage goes, whatever the test framework and however the command
/// is written. A runner's own `env` still wins on a name clash.
fn automatic_env(values: &BTreeMap<&'static str, String>) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("SKIES_EVIDENCE".to_string(), values["evidence"].clone()),
        ("SKIES_SPEC".to_string(), values["spec"].clone()),
        ("SKIES_COVERAGE".to_string(), values["coverage"].clone()),
    ])
}

fn path_text(path: &Path) -> Result<String> {
    path.to_str()
        .map(String::from)
        .with_context(|| format!("{} is not valid UTF-8", path.display()))
}

/// Replaces `{name}` for every known placeholder; unknown braces are left alone so shell syntax like `${VAR}` or
/// JSON in a command survives.
pub fn expand(template: &str, values: &BTreeMap<&'static str, String>) -> String {
    values.iter().fold(template.to_string(), |text, (key, value)| {
        text.replace(&format!("{{{key}}}"), value)
    })
}

/// Runs `command` through the platform shell, output captured to `log`. Returns whether it exited successfully.
fn shell(command: &str, cwd: &Path, env: &BTreeMap<String, String>, log: &Path) -> Result<bool> {
    let out = File::create(log).with_context(|| format!("creating {}", log.display()))?;
    let err = out.try_clone()?;
    let mut process = if cfg!(windows) {
        let mut process = Command::new("cmd");
        process.arg("/C").arg(command);
        process
    } else {
        let mut process = Command::new("sh");
        process.arg("-c").arg(command);
        process
    };
    let status = process
        .current_dir(cwd)
        .envs(env)
        .stdin(Stdio::null())
        .stdout(out)
        .stderr(err)
        .status()
        .with_context(|| format!("starting `{command}`"))?;
    Ok(status.success())
}

/// The last lines of a log, enough to see a build error without flooding the terminal.
fn tail(log: &Path) -> String {
    const LINES: usize = 30;
    let text = std::fs::read_to_string(log).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(LINES);
    lines[start..]
        .iter()
        .map(|line| format!("  | {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_known_placeholders_only() {
        let values = BTreeMap::from([("id", "0012".to_string()), ("report", "/tmp/r.xml".to_string())]);
        assert_eq!(
            expand(
                "test --filter S{id}. --out {report} ${HOME} {\"a\":1} {unknown}",
                &values
            ),
            "test --filter S0012. --out /tmp/r.xml ${HOME} {\"a\":1} {unknown}"
        );
    }
}
