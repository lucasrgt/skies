//! `skies proof run`: the spec's cases on the working tree, once, judged per failure mode.
//!
//! The loop an author runs while writing the cases and then the code: which modes pass, which fail, and the
//! runner's output when something does not. It is green without the bookkeeping: no receipt, no committed evidence.
//! The only files it writes are local, in the spec's evidence/raw/: the report (`run.xml` or `run.trx`), the output
//! (`run.log`), and what the cases saved under `$SKIES_EVIDENCE/raw/`. Everything else the run produces is gone when
//! it returns.

use anyhow::Result;

use super::evidence::{self, Files, RAW_DIR};
use super::green::{self, Mode};
use super::report::{Case, FmId, Outcome, case_fm};
use super::runner::seconds;
use super::spec::{self, EVIDENCE_DIR, SPECS_DIR, SpecDoc};
use crate::manifest::Project;

pub fn run(key: &str) -> Result<u8> {
    let project = Project::from_cwd()?;
    let root = project.root.as_path();
    let spec = spec::find(root, key)?;
    let doc = SpecDoc::load(&spec)?;
    if let Some(why) = doc.unprovable(&spec) {
        eprintln!("{why}");
        return Ok(1);
    }
    let runner_name = doc.runner(&spec)?;
    let scratch = tempfile::Builder::new().prefix("skies-proof-").tempdir()?;
    println!(
        "run {} (runner {runner_name}, working tree; writes only {EVIDENCE_DIR}/{RAW_DIR}/)",
        spec.name
    );
    let checked = green::check(&project, &spec, &doc, scratch.path())?;
    println!("  ran in {}", seconds(checked.run.elapsed));
    // The report, the output, and what the cases kept local stay for inspection; nothing committed is touched.
    let mut files = Files::default();
    files.add_saved(&checked.staged)?;
    let extension = checked.run.report.format.extension();
    files.raw.push((checked.run.file.clone(), format!("run.{extension}")));
    files.raw.push((checked.run.log.clone(), "run.log".to_string()));
    evidence::keep_local(&spec, &files.raw)?;
    println!(
        "  report and output in {}/{EVIDENCE_DIR}/{RAW_DIR}/ (local, not committed)",
        spec.rel()
    );
    if !evidence::is_ignored(root) {
        println!(
            "  note: {SPECS_DIR}/.gitignore does not ignore {EVIDENCE_DIR}/{RAW_DIR}/; `skies proof record` adds `{}`",
            evidence::IGNORE_RULE
        );
    }
    let modes = match &checked.modes {
        Ok(modes) => modes,
        Err(problems) => {
            eprintln!("{}: the run does not match spec.md:\n{problems}", spec.name);
            eprintln!("{}", checked.output());
            return Ok(1);
        }
    };
    for (id, mode) in modes {
        let text = doc.modes.get(id).map(|mode| mode.text.as_str()).unwrap_or_default();
        let verdict = match mode {
            Mode::Pass => "pass",
            Mode::CasesFailed | Mode::Unproven(_) => "fail",
        };
        println!("  {:<6}{verdict:<6}{}", id.to_string(), clip(text));
        match mode {
            Mode::Unproven(why) => println!("{INDENT}{why}"),
            Mode::CasesFailed => {
                for case in failed_cases(&checked.run.report.cases, *id) {
                    print_case(case);
                }
            }
            Mode::Pass => {}
        }
    }
    let failing = modes.values().filter(|mode| **mode != Mode::Pass).count();
    if failing == 0 {
        println!("{0}/{0} FMs pass", modes.len());
        return Ok(0);
    }
    println!("{failing} of {} FMs fail", modes.len());
    let failed = modes.values().any(|mode| *mode == Mode::CasesFailed);
    let explained = checked
        .run
        .report
        .cases
        .iter()
        .any(|case| case.outcome != Outcome::Passed && case.message.is_some());
    if failed && !explained {
        println!("{}", checked.output());
    }
    Ok(1)
}

const INDENT: &str = "              ";

/// The cases naming `id` that did not pass.
fn failed_cases(cases: &[Case], id: FmId) -> impl Iterator<Item = &Case> {
    cases
        .iter()
        .filter(move |case| case.outcome != Outcome::Passed && case_fm(&case.name) == Some(id))
}

/// A failed or skipped case and the first lines of what it reported.
fn print_case(case: &Case) {
    const LINES: usize = 8;
    let state = if case.outcome == Outcome::Skipped {
        "skipped"
    } else {
        "failed"
    };
    println!("{INDENT}{state}: {}", case.name);
    let Some(message) = &case.message else { return };
    let lines: Vec<&str> = message.lines().filter(|line| !line.trim().is_empty()).collect();
    for line in lines.iter().take(LINES) {
        println!("{INDENT}  | {}", line.trim_end());
    }
    if lines.len() > LINES {
        println!("{INDENT}  | …");
    }
}

/// A failure mode's text cut to one terminal line.
fn clip(text: &str) -> String {
    const WIDTH: usize = 90;
    if text.chars().count() <= WIDTH {
        return text.to_string();
    }
    let cut: String = text.chars().take(WIDTH - 1).collect();
    format!("{}…", cut.trim_end())
}
