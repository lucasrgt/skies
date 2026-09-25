//! `skies proof record`: red in a throwaway worktree, green in the working tree, then the receipt.
//!
//! Red answers "do these cases bite?": every failure mode must fail where the feature does not exist yet. Green
//! answers "does the feature handle them?". Only when both hold is a receipt written, so a receipt on disk always
//! describes a complete red→green pair.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};

use super::base::Base;
use super::evidence::{self, Files};
use super::git::Repo;
use super::green::{self, join};
use super::guard::{self, TestFiles};
use super::receipt::{self, Commands, GreenResult, Mode, Receipt, RedResult};
use super::red::{self, Revision};
use super::report::FmId;
use super::runner::seconds;
use super::spec::{self, EVIDENCE_DIR, RED_PATCH_FILE, SPEC_FILE, SpecDir, SpecDoc};
use super::{avp, impact};
use crate::manifest::Project;

pub fn record(key: &str, red_rev: Option<&str>, red_patch: Option<&Path>, allow_dirty: bool) -> Result<u8> {
    let project = Project::from_cwd()?;
    let root = project.root.as_path();
    let spec = spec::find(root, key)?;
    let doc = SpecDoc::load(&spec)?;
    if let Some(why) = doc.unprovable(&spec) {
        eprintln!("{why}");
        return Ok(1);
    }
    let runner_name = doc.runner(&spec)?;
    let runner = project.runner(runner_name)?;
    let repo = Repo::open(root)?;
    let dirty = guard::check_green(&repo, &spec, red_patch, allow_dirty)?;
    evidence::ensure_ignored(root)?;
    if !doc.touches.is_empty() {
        impact::warn_unmatched_touches(&spec, &doc, &impact::project_files(root))?;
    }

    let tests = TestFiles::of(&project, &repo, runner);
    let patch = choose_patch(&spec, red_rev, red_patch, |patch| {
        guard::check_patch(&tests, &repo, &spec, patch)
    })?;
    let head = repo.head()?;
    let base = match (red_rev, &patch) {
        (Some(rev), _) => Base::explicit(&repo, rev, "--red")?,
        (None, Some(_)) => Base {
            commit: head.clone(),
            how: format!("HEAD + {RED_PATCH_FILE}"),
            explicit: true,
        },
        (None, None) => Base::default(&repo, project.manifest.workspace.default_branch.as_deref())?,
    };
    if base.commit == head && patch.is_none() {
        bail!(
            "the red revision is HEAD ({}; {}), where the feature already exists, so red would prove nothing. Either\n  \
             - commit the feature on a branch and record from there (red defaults to the merge-base with the branch \
             features fork from: `default_branch` in Skies.toml, else origin/HEAD),\n  \
             - pass --red <rev> for a revision without the feature, or\n  \
             - pass --red-patch <file> with a patch that removes the feature (kept as {}/{RED_PATCH_FILE})",
            short(&head),
            base.how,
            spec.rel()
        );
    }

    // With a patch alone, red and green differ by the patch, which was vetted above.
    if patch.is_none() || red_rev.is_some() {
        guard::check_red_diff(&tests, &repo, &base.commit, red_rev.is_some())?;
    }

    let scratch = tempfile::Builder::new().prefix("skies-proof-").tempdir()?;
    println!("record {} (runner {runner_name})", spec.name);
    let (line, warning) = base.describe(&repo);
    println!("  red    {line}");
    if let Some(warning) = warning {
        eprintln!("{warning}");
    }
    let started = Instant::now();
    let revision = Revision {
        commit: &base.commit,
        patch: patch.as_deref(),
    };
    let runner = (runner_name, runner);
    let Some(red) = red::run(&repo, &spec, &doc, runner, &revision, scratch.path())? else {
        return Ok(1);
    };
    let red_elapsed = started.elapsed();
    if !justified(&spec, &doc, &red, &base, patch.is_some()) {
        return Ok(1);
    }

    println!("  green  {}", short(&head));
    let checked = green::check(&project, &spec, &doc, scratch.path())?;
    if let Some(problem) = green::refutation(&checked) {
        eprintln!("{}: {problem}", spec.name);
        return Ok(1);
    }
    println!(
        "  time   red {} (checkout and run), green {}",
        seconds(red_elapsed),
        seconds(checked.run.elapsed)
    );

    let mut files = Files::default();
    files.add_saved(&checked.staged)?;
    let mut modes = BTreeMap::new();
    for id in &doc.failure_modes {
        let tagged = !doc.avp(*id).is_empty();
        let verdict = avp::verdict_file(*id);
        let red_verdict = red.verdict(&doc, *id).map(|file| {
            files.committed.push((file, format!("red.{verdict}")));
            format!("{EVIDENCE_DIR}/red.{verdict}")
        });
        modes.insert(
            *id,
            Mode {
                red: red.results[id],
                green: GreenResult::Pass,
                cases: checked.names(*id),
                message: red.messages.get(id).cloned(),
                avp: doc.avp(*id).to_vec(),
                verdict: tagged.then(|| format!("{EVIDENCE_DIR}/{verdict}")),
                red_verdict,
            },
        );
    }
    let green_ext = checked.run.report.format.extension();
    files.raw.push((checked.run.file.clone(), format!("green.{green_ext}")));
    files.raw.push((checked.run.log.clone(), "green.log".to_string()));
    files.raw.push((red.log.clone(), "red.log".to_string()));
    if let Some((file, extension)) = &red.report {
        files.raw.push((file.clone(), format!("red.{extension}")));
    }
    if let Err(error) = evidence::publish(&spec, &files) {
        eprintln!("{}: {error:#}", spec.name);
        return Ok(1);
    }

    print_modes(&modes);
    if modes.values().all(|mode| mode.red == RedResult::DidNotBuild) {
        eprintln!(
            "warning: every failure mode is did-not-build, so this red proves only that the spec's e2e use types or \
             modules the feature adds, not that any assertion fails without its behavior. To prove the behavior too, \
             record with a red.patch that keeps the new types but stubs what they do (`--red-patch <file>`)."
        );
    }
    let receipt = Receipt {
        spec: spec.name.clone(),
        runner: runner_name.to_string(),
        commands: Commands::of(runner.1),
        red: receipt::Red {
            commit: base.commit.clone(),
            patch: patch.as_ref().map(|_| RED_PATCH_FILE.to_string()),
            output: red.output,
        },
        green: receipt::Green { commit: head, dirty },
        failure_modes: modes,
    };
    receipt.save(&spec)?;
    println!(
        "wrote {}/receipt.json (full reports in {EVIDENCE_DIR}/{}/, not committed)",
        spec.rel(),
        evidence::RAW_DIR
    );
    Ok(0)
}

/// Whether every mode that passed red is justified under `## Non-discriminating`; says what to write when not.
fn justified(spec: &SpecDir, doc: &SpecDoc, red: &red::Red, base: &Base, patched: bool) -> bool {
    let unjustified: Vec<FmId> = red
        .results
        .iter()
        .filter(|(id, result)| **result == RedResult::NonDiscriminating && !doc.justified.contains(id))
        .map(|(id, _)| *id)
        .collect();
    if unjustified.is_empty() {
        return true;
    }
    let patch_note = if patched {
        format!(" + {RED_PATCH_FILE}")
    } else {
        String::new()
    };
    eprintln!(
        "{}: {} already pass on red ({}{patch_note}), so their cases do not prove the feature.",
        spec.name,
        join(&unjustified),
        short(&base.commit)
    );
    eprintln!(
        "Make each case fail without the feature, or justify it in {}/{SPEC_FILE}:\n",
        spec.rel()
    );
    eprintln!("## Non-discriminating\n");
    for id in &unjustified {
        eprintln!("- {id} <why this case cannot fail before the feature>");
    }
    false
}

/// Picks the patch that turns the red revision into "feature not implemented": `--red-patch` (stored as the spec's
/// red.patch so the receipt is reproducible), else the spec's own red.patch when `--red` does not override it. The
/// patch is vetted before it is stored, so a refused one never replaces the spec's red.patch.
fn choose_patch(
    spec: &SpecDir,
    red_rev: Option<&str>,
    given: Option<&Path>,
    vet: impl Fn(&Path) -> Result<()>,
) -> Result<Option<PathBuf>> {
    let stored = spec.file(RED_PATCH_FILE);
    match given {
        Some(given) => {
            if !given.is_file() {
                bail!("--red-patch {}: no such file", given.display());
            }
            vet(given)?;
            if std::fs::canonicalize(given).ok() != std::fs::canonicalize(&stored).ok() {
                std::fs::copy(given, &stored)
                    .with_context(|| format!("copying {} to {}", given.display(), stored.display()))?;
            }
            Ok(Some(stored))
        }
        None if red_rev.is_none() && stored.is_file() => {
            vet(&stored)?;
            Ok(Some(stored))
        }
        None => Ok(None),
    }
}

/// Red and green per failure mode.
fn print_modes(modes: &BTreeMap<FmId, Mode>) {
    println!("  {:<6}{:<20}green", "FM", "red");
    for (id, mode) in modes {
        let red = match mode.red {
            RedResult::Fail => "fail",
            RedResult::DidNotBuild => "did not build",
            RedResult::NonDiscriminating => "non-discriminating",
        };
        let tag = if mode.avp.is_empty() {
            String::new()
        } else {
            format!("  [avp: {}]", mode.avp.join(", "))
        };
        println!("  {:<6}{red:<20}pass{tag}", id.to_string());
    }
}

pub fn short(commit: &str) -> &str {
    &commit[..commit.len().min(7)]
}
