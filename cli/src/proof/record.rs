//! `skies proof record`: red in a throwaway worktree, green in the working tree, then the receipt.
//!
//! Red answers "do these cases bite?": every failure mode must fail where the feature does not exist yet. Green
//! answers "does the feature handle them?". Only when both hold is a receipt written, so a receipt on disk always
//! describes a complete red→green pair.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use super::git::Repo;
use super::hash;
use super::receipt::{Green, GreenCase, Patch, Receipt, Red, RedCase};
use super::report::{self, FmId};
use super::runner::{Job, Session};
use super::spec::{self, E2E_DIR, EVIDENCE_DIR, RED_PATCH_FILE, SPEC_FILE, SpecDir, SpecDoc};
use crate::manifest::Project;

pub fn record(key: &str, red_flag: Option<&str>, red_patch_flag: Option<&Path>) -> Result<u8> {
    let project = Project::from_cwd()?;
    let root = project.root.as_path();
    let spec = spec::find(root, key)?;
    let doc = SpecDoc::load(&spec)?;
    let runner_name = doc.runner(&spec)?;
    let runner = project.runner(runner_name)?;
    let repo = Repo::open(root)?;

    let patch = red_patch(&spec, red_flag, red_patch_flag)?;
    let head = repo.head()?;
    let red_commit = match (red_flag, &patch) {
        (Some(rev), _) => repo.resolve(rev)?,
        (None, Some(_)) => head.clone(),
        (None, None) => repo.fork_point()?,
    };
    if red_commit == head && patch.is_none() {
        bail!(
            "the red revision is HEAD ({}), where the feature already exists, so red would prove nothing. Either\n  \
             - commit the feature on a branch and record from there (red defaults to the merge-base with the default branch),\n  \
             - pass --red <rev> for a revision without the feature, or\n  \
             - pass --red-patch <file> with a patch that removes the feature (kept as {}/{RED_PATCH_FILE})",
            short(&head),
            spec.rel()
        );
    }

    let scratch = tempfile::Builder::new().prefix("skies-proof-").tempdir()?;
    let mut session = Session::default();
    let patch_note = if patch.is_some() {
        format!(" + {RED_PATCH_FILE}")
    } else {
        String::new()
    };
    println!("record {} (runner {runner_name})", spec.name);
    println!("  red    {}{patch_note}", short(&red_commit));

    // Red, in a worktree that is gone again before green starts.
    let red_run = {
        let worktree = repo.temp_worktree(&red_commit)?;
        let red_root = worktree.path.join(&repo.prefix);
        let red_spec = SpecDir {
            path: red_root.join(spec.rel()),
            ..spec.clone()
        };
        copy_spec_sources(&spec, &red_spec)?;
        if let Some(patch) = &patch {
            repo.apply(&worktree.path, patch)?;
        }
        let evidence = scratch.path().join("red-evidence");
        let run = session.run(&Job {
            runner_name,
            runner,
            spec: &red_spec,
            root: &red_root,
            evidence: &evidence,
            scratch: scratch.path(),
            label: "red",
        })?;
        // A configured report path lives inside the worktree; keep the report past its removal.
        let kept = scratch.path().join("red.report");
        std::fs::copy(&run.file, &kept)?;
        (run.report, kept)
    };
    let red_eval = match report::evaluate(&doc.failure_modes, &red_run.0.cases) {
        Ok(evaluation) => evaluation,
        Err(problems) => {
            eprintln!("{}: the red run does not match spec.md:\n{problems}", spec.name);
            eprintln!(
                "If the e2e cannot compile at {}, record against a patch instead: --red-patch <file>.",
                short(&red_commit)
            );
            return Ok(1);
        }
    };
    let red_cases: BTreeMap<FmId, RedCase> = red_eval
        .passed
        .iter()
        .map(|(id, passed)| {
            (
                *id,
                if *passed {
                    RedCase::NonDiscriminating
                } else {
                    RedCase::Fail
                },
            )
        })
        .collect();
    let unjustified: Vec<FmId> = red_cases
        .iter()
        .filter(|(id, case)| **case == RedCase::NonDiscriminating && !doc.justified.contains(id))
        .map(|(id, _)| *id)
        .collect();
    if !unjustified.is_empty() {
        let ids = join(&unjustified);
        eprintln!(
            "{}: {ids} already pass on red ({}{patch_note}), so their cases do not prove the feature.",
            spec.name,
            short(&red_commit)
        );
        eprintln!(
            "Make each case fail without the feature, or justify it in {}/{SPEC_FILE}:\n",
            spec.rel()
        );
        eprintln!("## Non-discriminating\n");
        for id in &unjustified {
            eprintln!("- {id} <why this case cannot fail before the feature>");
        }
        return Ok(1);
    }

    let dirty = repo.dirty()?;
    println!("  green  {}{}", short(&head), if dirty { " (dirty)" } else { "" });
    let green = match run_green(root, &spec, &doc, &project, &mut session, scratch.path())? {
        GreenOutcome::Proven(green) => green,
        GreenOutcome::Refuted(message) => {
            eprintln!("{}: {message}", spec.name);
            return Ok(1);
        }
    };
    print_cases(&red_cases);

    let red_report = format!("{EVIDENCE_DIR}/red.{}", red_run.0.format.extension());
    publish_evidence(
        &spec,
        &green.staged,
        &[(&red_run.1, &red_report), (&green.file, &green.report)],
        false,
    )?;

    let footprint_paths = footprint(&repo, root, &doc, &red_commit, patch.as_deref())?;
    let receipt = Receipt {
        spec: spec.name.clone(),
        runner: runner_name.to_string(),
        red: Red {
            commit: red_commit,
            patch: patch.as_deref().map(|file| Patch {
                file: RED_PATCH_FILE.to_string(),
                hash: hash::hash_file(file).unwrap_or_else(|| hash::ABSENT.to_string()),
            }),
            cases: red_cases,
            report: red_report,
        },
        green: Green {
            commit: head,
            dirty,
            cases: green.cases,
            report: green.report,
        },
        footprint: hash::hash_all(root, &footprint_paths),
        inputs: hash::hash_all(root, &hash::input_paths(root, &spec)?),
    };
    receipt.save(&spec)?;
    println!(
        "wrote {}/receipt.json (footprint {} files, inputs {} files)",
        spec.rel(),
        receipt.footprint.len(),
        receipt.inputs.len()
    );
    Ok(0)
}

/// Picks the patch that turns the red revision into "feature not implemented": `--red-patch` (stored as the spec's
/// red.patch so the receipt is reproducible), else the spec's own red.patch when no red flag overrides it.
fn red_patch(spec: &SpecDir, red_flag: Option<&str>, given: Option<&Path>) -> Result<Option<PathBuf>> {
    let stored = spec.file(RED_PATCH_FILE);
    match given {
        Some(given) => {
            if !given.is_file() {
                bail!("--red-patch {}: no such file", given.display());
            }
            let same = std::fs::canonicalize(given).ok() == std::fs::canonicalize(&stored).ok();
            if !same {
                std::fs::copy(given, &stored)
                    .with_context(|| format!("copying {} to {}", given.display(), stored.display()))?;
            }
            Ok(Some(stored))
        }
        None if red_flag.is_none() && stored.is_file() => Ok(Some(stored)),
        None => Ok(None),
    }
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
        copy_dir(&from.file(E2E_DIR), &e2e)?;
    }
    Ok(())
}

/// What changed between red and the working tree, plus `touches`, minus every spec folder.
fn footprint(repo: &Repo, root: &Path, doc: &SpecDoc, red: &str, patch: Option<&Path>) -> Result<BTreeSet<String>> {
    let changed = match patch {
        Some(patch) => repo.patch_footprint(patch)?,
        None => repo.changed_since(red)?,
    };
    let mut paths: BTreeSet<String> = changed.into_iter().collect();
    paths.extend(hash::touched_paths(root, &doc.touches)?);
    paths.retain(|path| !hash::is_spec_path(path));
    Ok(paths)
}

pub struct ProvenGreen {
    pub cases: BTreeMap<FmId, GreenCase>,
    /// The report file name inside evidence/, relative to the spec folder.
    pub report: String,
    /// The report as the runner wrote it.
    pub file: PathBuf,
    /// Artifacts the runner wrote to `{evidence}`, staged until the run is known to be good.
    pub staged: PathBuf,
}

pub enum GreenOutcome {
    Proven(ProvenGreen),
    /// The run finished but does not prove the spec; the message says why.
    Refuted(String),
}

/// Runs the spec on the working tree. Evidence is staged in `scratch` so a failing run never overwrites the
/// evidence of the last good one.
pub fn run_green(
    root: &Path,
    spec: &SpecDir,
    doc: &SpecDoc,
    project: &Project,
    session: &mut Session,
    scratch: &Path,
) -> Result<GreenOutcome> {
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
    let evaluation = match report::evaluate(&doc.failure_modes, &run.report.cases) {
        Ok(evaluation) => evaluation,
        Err(problems) => {
            return Ok(GreenOutcome::Refuted(format!(
                "the green run does not match spec.md:\n{problems}"
            )));
        }
    };
    let failing: Vec<FmId> = evaluation
        .passed
        .iter()
        .filter(|(_, passed)| !**passed)
        .map(|(id, _)| *id)
        .collect();
    if !failing.is_empty() {
        return Ok(GreenOutcome::Refuted(format!(
            "{} not passing on the working tree (a failed or skipped case counts as not passing)",
            join(&failing)
        )));
    }
    Ok(GreenOutcome::Proven(ProvenGreen {
        cases: evaluation.passed.keys().map(|id| (*id, GreenCase::Pass)).collect(),
        report: format!("{EVIDENCE_DIR}/green.{}", run.report.format.extension()),
        file: run.file,
        staged,
    }))
}

/// Replaces evidence/ with the staged runner artifacts plus the given reports (`(source, path in spec folder)`).
/// With `keep_red`, the red report of the original recording survives, since `verify` never reruns red.
pub fn publish_evidence(spec: &SpecDir, staged: &Path, reports: &[(&Path, &str)], keep_red: bool) -> Result<()> {
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
    for (source, target) in reports {
        std::fs::copy(source, spec.path.join(target)).with_context(|| format!("copying the report to {target}"))?;
    }
    Ok(())
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

/// Red outcome per failure mode; green is only printed on success, where every one of them passed.
fn print_cases(red: &BTreeMap<FmId, RedCase>) {
    println!("  {:<6}{:<20}green", "FM", "red");
    for (id, case) in red {
        let red_text = match case {
            RedCase::Fail => "fail",
            RedCase::NonDiscriminating => "non-discriminating",
        };
        println!("  {:<6}{red_text:<20}pass", id.to_string());
    }
}

pub fn short(commit: &str) -> &str {
    &commit[..commit.len().min(7)]
}

pub fn join(ids: &[FmId]) -> String {
    ids.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")
}
