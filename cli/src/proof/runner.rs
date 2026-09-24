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

/// A finished run: the parsed report and the file it came from, which the caller copies into evidence verbatim.
pub struct Run {
    pub report: Report,
    pub file: PathBuf,
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
        let env: BTreeMap<String, String> =
            job.runner.env.iter().map(|(key, value)| (key.clone(), expand(value, &values))).collect();

        if let Some(setup) = &job.runner.setup
            && self.done.insert((job.runner_name.to_string(), job.root.to_path_buf()))
        {
            let log = job.scratch.join(format!("{}-setup.log", job.label));
            let status = shell(&expand(setup, &values), job.root, &env, &log)?;
            if !status {
                bail!("runner '{}' setup failed on {}:\n{}", job.runner_name, job.label, tail(&log));
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
        std::fs::create_dir_all(job.evidence)?;

        let log = job.scratch.join(format!("{}.log", job.label));
        shell(&expand(&job.runner.command, &values), job.root, &env, &log)?;
        let text = std::fs::read_to_string(&report_path).with_context(|| {
            format!(
                "runner '{}' wrote no report at {} on {} (did the tests build?). Last output:\n{}",
                job.runner_name,
                report_path.display(),
                job.label,
                tail(&log)
            )
        })?;
        let report = report::parse(&text)
            .with_context(|| format!("reading the {} report {}", job.label, report_path.display()))?;
        Ok(Run { report, file: report_path })
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
    Ok(values)
}

fn path_text(path: &Path) -> Result<String> {
    path.to_str().map(String::from).with_context(|| format!("{} is not valid UTF-8", path.display()))
}

/// Replaces `{name}` for every known placeholder; unknown braces are left alone so shell syntax like `${VAR}` or
/// JSON in a command survives.
pub fn expand(template: &str, values: &BTreeMap<&'static str, String>) -> String {
    values.iter().fold(template.to_string(), |text, (key, value)| text.replace(&format!("{{{key}}}"), value))
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
    lines[start..].iter().map(|line| format!("  | {line}")).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_known_placeholders_only() {
        let values = BTreeMap::from([("id", "0012".to_string()), ("report", "/tmp/r.xml".to_string())]);
        assert_eq!(
            expand("test --filter S{id}. --out {report} ${HOME} {\"a\":1} {unknown}", &values),
            "test --filter S0012. --out /tmp/r.xml ${HOME} {\"a\":1} {unknown}"
        );
    }
}
