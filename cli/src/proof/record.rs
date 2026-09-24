//! `skies proof record`: red in a throwaway worktree, green in the working tree, then the receipt.
//!
//! Red answers "do these cases bite?": every failure mode must fail where the feature does not exist yet. Green
//! answers "does the feature handle them?". Only when both hold is a receipt written, so a receipt on disk always
//! describes a complete red→green pair.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};

use super::base::Base;
use super::git::Repo;
use super::green::{self, GreenOutcome, join};
use super::hash;
use super::receipt::{Entry, Green, Patch, Receipt, Red, RedCase};
use super::red::{self, Revision};
use super::report::FmId;
use super::runner::{Session, seconds};
use super::spec::{self, EVIDENCE_DIR, RED_PATCH_FILE, SPEC_FILE, SpecDir, SpecDoc};
use super::{avp, ctx, footprint, impact, lines, verify};
use crate::manifest::{Project, Runner};

/// What `skies proof record` was asked to do.
pub struct Options<'a> {
    pub red: Option<&'a str>,
    pub red_patch: Option<&'a Path>,
    /// Re-prove green for every other spec whose footprint overlaps this one's, and name them in the receipt.
    pub with_impacted: bool,
}

pub fn record(key: &str, options: &Options) -> Result<u8> {
    let project = Project::from_cwd()?;
    let root = project.root.as_path();
    let spec = spec::find(root, key)?;
    let doc = SpecDoc::load(&spec)?;
    let runner_name = doc.runner(&spec)?;
    let runner = project.runner(runner_name)?;
    let repo = Repo::open(root)?;

    let patch = red_patch(&spec, options.red, options.red_patch)?;
    let head = repo.head()?;
    // --red, then the spec's red.patch on HEAD, then the merge-base with the branch features fork from.
    let base = match (options.red, &patch) {
        (Some(rev), _) => Base::explicit(&repo, rev, "--red")?,
        (None, Some(_)) => Base {
            commit: head.clone(),
            how: format!("HEAD + {RED_PATCH_FILE}"),
            explicit: true,
        },
        (None, None) => Base::default(&repo, project.manifest.workspace.default_branch.as_deref())?,
    };
    let red_commit = base.commit.clone();
    if red_commit == head && patch.is_none() {
        bail!(
            "the red revision is HEAD ({}; {}), where the feature already exists, so red would prove nothing. Either\n  \
             - commit the feature on a branch and record from there (red defaults to the merge-base with the branch \
             features fork from: `default_branch` in Skies.toml, else the upstream, else origin/HEAD),\n  \
             - pass --red <rev> for a revision without the feature, or\n  \
             - pass --red-patch <file> with a patch that removes the feature (kept as {}/{RED_PATCH_FILE})",
            short(&head),
            base.how,
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
    let (line, warning) = base.describe(&repo);
    println!("  red    {line}");
    if let Some(warning) = warning {
        eprintln!("{warning}");
    }

    let revision = Revision {
        commit: &red_commit,
        patch: patch.as_deref(),
    };
    let red_started = Instant::now();
    let Some(red_run) = red::run(
        &repo,
        &spec,
        &doc,
        (runner_name, runner),
        &revision,
        &mut session,
        scratch.path(),
    )?
    else {
        return Ok(1);
    };
    let red_elapsed = red_started.elapsed();
    let unjustified: Vec<FmId> = red_run
        .cases
        .iter()
        .filter(|(id, case)| case.result() == RedCase::NonDiscriminating && !doc.justified.contains(id))
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
    let proven = match green::run_green(root, &spec, &doc, &project, &mut session, scratch.path())? {
        GreenOutcome::Proven(proven) => proven,
        GreenOutcome::Refuted(message) => {
            eprintln!("{}: {message}", spec.name);
            return Ok(1);
        }
    };
    println!(
        "  time   red {} (checkout and run), green {}",
        seconds(red_elapsed),
        seconds(proven.elapsed)
    );
    print_cases(&red_run.cases);

    let mut files = vec![
        (red_run.file.clone(), red_run.report.clone()),
        (proven.file.clone(), proven.report.clone()),
    ];
    for id in &doc.failure_modes {
        let name = avp::verdict_file(*id);
        if !doc.avp(*id).is_empty() && red_run.evidence.join(&name).is_file() {
            files.push((red_run.evidence.join(&name), format!("{EVIDENCE_DIR}/red.{name}")));
        }
    }
    green::publish_evidence(&spec, &proven.staged, &files, false, &red_run.scrub)?;

    let changed = match patch.as_deref() {
        Some(patch) => repo.patch_footprint(patch)?,
        None => repo.changed_since(&red_commit)?,
    };
    let footprint = footprint::build(root, &doc, runner, &changed, &proven)?;
    let prints = lines::print_all(root, &footprint.paths, &footprint.executed);
    println!(
        "  {}",
        footprint::describe(&footprint, &proven, lines::by_lines(&prints))
    );
    let footprint_paths = footprint.paths;
    let ctx_revised = revised_ctx(&repo, runner, &changed, patch.is_some().then_some(head.as_str()))?;
    let mut receipt = Receipt {
        spec: spec.name.clone(),
        runner: runner_name.to_string(),
        red: Red {
            commit: red_commit,
            patch: patch.as_deref().map(|file| Patch {
                file: RED_PATCH_FILE.to_string(),
                hash: hash::hash_file(file).unwrap_or_else(|| hash::ABSENT.to_string()),
            }),
            cases: red_run.cases,
            report: red_run.report,
        },
        green: Green {
            commit: head,
            dirty,
            cases: proven.cases,
            report: proven.report,
        },
        footprint: prints,
        footprint_source: footprint.source,
        footprint_changed: footprint.changed,
        inputs: hash::hash_all(root, &hash::input_paths(root, &spec)?),
        evidence: Some(green::evidence_hashes(&spec, None)?),
        ctx_revised,
        verified_with: BTreeMap::new(),
    };
    receipt.save(&spec)?;
    println!(
        "wrote {}/receipt.json (footprint {}, inputs {}, evidence {})",
        spec.rel(),
        footprint::files(receipt.footprint.len()),
        footprint::files(receipt.inputs.len()),
        footprint::files(receipt.evidence.as_ref().map_or(0, |evidence| evidence.len()))
    );
    note_unrevised_ctx(root, &spec, &footprint_paths, &receipt.ctx_revised);
    impacted(&project, &spec, &mut receipt, options.with_impacted, &mut session)
}

/// A spec that touched a module whose ctx.md stayed as it was gets a note, never a failure: the change may well
/// have left every invariant intact, and only the author can tell. The footprint is already limited to the
/// runner's scope, so a screen's spec never hears about a backend module's notes.
fn note_unrevised_ctx(root: &Path, spec: &SpecDir, footprint: &BTreeSet<String>, revised: &[String]) {
    for file in ctx::touched(root, footprint) {
        if !revised.contains(&file) {
            println!(
                "note: {} was not revised in this change; update its design notes and cite this spec (`{}#FM-n`) if an invariant changed.",
                ctx::file_name(&file),
                spec.name
            );
        }
    }
}

/// The ctx files in the runner's scope revised in this change: those in the red..working-tree diff and, when red
/// is HEAD plus a red.patch (which removes the feature and rarely touches prose), those edited in the working tree
/// since HEAD.
fn revised_ctx(repo: &Repo, runner: &Runner, changed: &[String], patched_head: Option<&str>) -> Result<Vec<String>> {
    let mut revised: BTreeSet<String> = changed.iter().filter(|path| ctx::is_ctx(path)).cloned().collect();
    if let Some(head) = patched_head {
        revised.extend(repo.changed_since(head)?.into_iter().filter(|path| ctx::is_ctx(path)));
    }
    Ok(revised.into_iter().filter(|path| runner.in_scope(path)).collect())
}

/// Lists the other specs this receipt's footprint reaches and, with `--with-impacted`, re-proves them green. The
/// new receipt is already written; `verified_with` is added only for specs that passed, and any failure makes the
/// exit code 1 so the author sees that the change broke a neighbor. A spec with no receipt yet is `unrecorded`:
/// there is nothing to re-prove, and it breaks nothing.
fn impacted(
    project: &Project,
    spec: &SpecDir,
    receipt: &mut Receipt,
    rerun: bool,
    session: &mut Session,
) -> Result<u8> {
    let index = impact::index(&project.root)?;
    let paths: BTreeSet<String> = receipt.footprint.keys().cloned().collect();
    let specs: Vec<SpecDir> = impact::impacted_by(&index, &spec.name, &paths)
        .into_iter()
        .map(|entry| entry.spec.clone())
        .collect();
    if specs.is_empty() {
        return Ok(0);
    }
    let names: Vec<&str> = specs.iter().map(|spec| spec.name.as_str()).collect();
    if !rerun {
        println!("impacted: {} share files with this footprint", names.join(", "));
        println!(
            "  rerun them with `skies proof record {} --with-impacted` (or `skies proof verify {}`)",
            spec.id,
            specs.iter().map(|spec| spec.id.as_str()).collect::<Vec<_>>().join(" ")
        );
        return Ok(0);
    }
    println!("verify impacted: {}", names.join(", "));
    // The same session as green, so the runner does not set up or build the working tree a second time.
    let outcomes = verify::verify_specs(project, &specs, false, session)?;
    verify::print_outcomes(&outcomes);
    receipt.verified_with = outcomes
        .iter()
        .filter_map(|outcome| Some((outcome.name.clone(), outcome.receipt()?.to_string())))
        .collect();
    receipt.save(spec)?;
    let failed: Vec<&str> = outcomes
        .iter()
        .filter(|outcome| outcome.failed())
        .map(|outcome| outcome.name.as_str())
        .collect();
    if failed.is_empty() {
        return Ok(0);
    }
    eprintln!(
        "{}: recorded, but this change breaks impacted spec{} {} (their receipts are unchanged)",
        spec.name,
        if failed.len() == 1 { "" } else { "s" },
        failed.join(", ")
    );
    Ok(1)
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

/// Red outcome per failure mode; green is only printed on success, where every one of them passed.
fn print_cases(red: &BTreeMap<FmId, Entry<RedCase>>) {
    println!("  {:<6}{:<20}green", "FM", "red");
    for (id, case) in red {
        let red_text = match case.result() {
            RedCase::Fail => "fail",
            RedCase::DidNotBuild => "did not build",
            RedCase::NonDiscriminating => "non-discriminating",
        };
        let tag = match case {
            Entry::Avp(entry) => format!("  [avp: {}]", entry.avp.join(", ")),
            Entry::Plain(_) => String::new(),
        };
        println!("  {:<6}{red_text:<20}pass{tag}", id.to_string());
    }
}

pub fn short(commit: &str) -> &str {
    &commit[..commit.len().min(7)]
}
