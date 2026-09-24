//! `skies proof record`: red in a throwaway worktree, green in the working tree, then the receipt.
//!
//! Red answers "do these cases bite?": every failure mode must fail where the feature does not exist yet. Green
//! answers "does the feature handle them?". Only when both hold is a receipt written, so a receipt on disk always
//! describes a complete red→green pair. `--red-only` ([`super::red_only`]) reruns red alone for a receipt whose red
//! rotted.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use super::base::Base;
use super::evidence::{self, Half};
use super::git::Repo;
use super::green::{self, GreenOutcome, join};
use super::hash;
use super::receipt::{Entry, Green, Receipt, RedCase};
use super::red::{self, Revision};
use super::report::FmId;
use super::runner::{Session, seconds};
use super::spec::{self, RED_PATCH_FILE, SPEC_FILE, SpecDir, SpecDoc};
use super::{ctx, footprint, impact, lines, red_only, verify};
use crate::manifest::{Project, Runner};

/// What `skies proof record` was asked to do.
pub struct Options<'a> {
    pub red: Option<&'a str>,
    pub red_patch: Option<&'a Path>,
    /// Re-prove green for every other spec whose footprint overlaps this one's, and name them in the receipt.
    pub with_impacted: bool,
    /// Rerun red alone and rewrite only the receipt's red half: the fix for a red.patch that rotted.
    pub red_only: bool,
}

pub fn record(key: &str, options: &Options) -> Result<u8> {
    let project = Project::from_cwd()?;
    let root = project.root.as_path();
    let spec = spec::find(root, key)?;
    let doc = SpecDoc::load(&spec)?;
    let runner_name = doc.runner(&spec)?;
    let runner = project.runner(runner_name)?;
    let repo = Repo::open(root)?;
    if options.red_only {
        return red_only::record(&project, &spec, &doc, &repo, options);
    }
    evidence::ensure_ignored(root)?;
    let plan = plan_red(&project, &spec, &repo, options)?;

    let scratch = tempfile::Builder::new().prefix("skies-proof-").tempdir()?;
    let mut session = Session::default();
    println!("record {} (runner {runner_name})", spec.name);
    let Some((red_run, red_elapsed)) = prove_red(
        &repo,
        &spec,
        &doc,
        (runner_name, runner),
        &plan,
        &mut session,
        scratch.path(),
    )?
    else {
        return Ok(1);
    };

    let dirty = repo.dirty()?;
    println!("  green  {}{}", short(&plan.head), if dirty { " (dirty)" } else { "" });
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
    print_cases(&red_run.cases, "pass");

    {
        let red_files = red_run.files(&doc);
        let green_files = proven.files();
        for files in [&red_files, &green_files] {
            if let Some(problem) = evidence::oversized(&repo, &spec, files)? {
                eprintln!("{}: {problem}", spec.name);
                return Ok(1);
            }
        }
        evidence::publish(&spec, Half::Red, &red_files, &red_run.scrub)?;
        evidence::publish(&spec, Half::Green, &green_files, &red_run.scrub)?;
    }

    let changed = match plan.patch.as_deref() {
        Some(patch) => repo.patch_footprint(patch)?,
        None => repo.changed_since(&plan.base.commit)?,
    };
    let footprint = footprint::build(root, &doc, runner, &changed, &proven)?;
    let prints = lines::print_all(root, &footprint.paths, &footprint.executed);
    println!(
        "  {}",
        footprint::describe(&footprint, &proven, lines::by_lines(&prints))
    );
    let footprint_paths = footprint.paths;
    let patched_head = plan.patch.is_some().then_some(plan.head.as_str());
    let ctx_revised = revised_ctx(&repo, runner, &changed, patched_head)?;
    let green_report = proven.report(&spec);
    let mut receipt = Receipt {
        spec: spec.name.clone(),
        runner: runner_name.to_string(),
        red: red_run.into_receipt(&spec, plan.base.commit.clone(), plan.patch.as_deref()),
        green: Green {
            commit: plan.head.clone(),
            dirty,
            cases: proven.cases,
            report: green_report,
        },
        footprint: prints,
        footprint_source: footprint.source,
        footprint_changed: footprint.changed,
        inputs: hash::hash_all(root, &hash::input_paths(root, &spec)?),
        evidence: Some(evidence::hashes(&spec, None, None, &[])?),
        ctx_revised,
        verified_with: BTreeMap::new(),
    };
    receipt.save(&spec)?;
    println!(
        "wrote {}/receipt.json (footprint {}, inputs {}, evidence {}; full reports in evidence/{}/, not committed)",
        spec.rel(),
        footprint::files(receipt.footprint.len()),
        footprint::files(receipt.inputs.len()),
        footprint::files(receipt.evidence.as_ref().map_or(0, |evidence| evidence.len())),
        evidence::RAW_DIR
    );
    note_unrevised_ctx(root, &spec, &footprint_paths, &receipt.ctx_revised);
    impacted(&project, &spec, &mut receipt, options.with_impacted, &mut session)
}

/// Where red runs, decided before anything runs.
pub struct RedPlan {
    pub base: Base,
    pub patch: Option<PathBuf>,
    pub head: String,
}

/// `--red`, then the spec's red.patch on HEAD, then the merge-base with the branch features fork from. Refuses a
/// red that is HEAD itself without a patch, where the feature already exists.
pub fn plan_red(project: &Project, spec: &SpecDir, repo: &Repo, options: &Options) -> Result<RedPlan> {
    let patch = red_patch(spec, options.red, options.red_patch)?;
    let head = repo.head()?;
    let base = match (options.red, &patch) {
        (Some(rev), _) => Base::explicit(repo, rev, "--red")?,
        (None, Some(_)) => Base {
            commit: head.clone(),
            how: format!("HEAD + {RED_PATCH_FILE}"),
            explicit: true,
        },
        (None, None) => Base::default(repo, project.manifest.workspace.default_branch.as_deref())?,
    };
    if base.commit == head && patch.is_none() {
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
    Ok(RedPlan { base, patch, head })
}

/// Prints the red line, runs red, and holds it to spec.md: `None` (after saying why) when red does not match it or a
/// mode passes red without a justification.
pub fn prove_red(
    repo: &Repo,
    spec: &SpecDir,
    doc: &SpecDoc,
    runner: (&str, &Runner),
    plan: &RedPlan,
    session: &mut Session,
    scratch: &Path,
) -> Result<Option<(red::Red, Duration)>> {
    let (line, warning) = plan.base.describe(repo);
    println!("  red    {line}");
    if let Some(warning) = warning {
        eprintln!("{warning}");
    }
    let revision = Revision {
        commit: &plan.base.commit,
        patch: plan.patch.as_deref(),
    };
    let started = Instant::now();
    let Some(red_run) = red::run(repo, spec, doc, runner, &revision, session, scratch)? else {
        return Ok(None);
    };
    let elapsed = started.elapsed();
    let unjustified: Vec<FmId> = red_run
        .cases
        .iter()
        .filter(|(id, case)| case.result() == RedCase::NonDiscriminating && !doc.justified.contains(id))
        .map(|(id, _)| *id)
        .collect();
    if unjustified.is_empty() {
        return Ok(Some((red_run, elapsed)));
    }
    let patch_note = if plan.patch.is_some() {
        format!(" + {RED_PATCH_FILE}")
    } else {
        String::new()
    };
    eprintln!(
        "{}: {} already pass on red ({}{patch_note}), so their cases do not prove the feature.",
        spec.name,
        join(&unjustified),
        short(&plan.base.commit)
    );
    eprintln!(
        "Make each case fail without the feature, or justify it in {}/{SPEC_FILE}:\n",
        spec.rel()
    );
    eprintln!("## Non-discriminating\n");
    for id in &unjustified {
        eprintln!("- {id} <why this case cannot fail before the feature>");
    }
    Ok(None)
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

/// Red outcome per failure mode, next to green's (`pass` after a full record, `kept` after `--red-only`).
pub fn print_cases(red: &BTreeMap<FmId, Entry<RedCase>>, green: &str) {
    println!("  {:<6}{:<20}green", "FM", "red");
    for (id, case) in red {
        let red_text = match case.result() {
            RedCase::Fail => "fail",
            RedCase::DidNotBuild => "did not build",
            RedCase::NonDiscriminating => "non-discriminating",
        };
        let tag = if case.avp.is_empty() {
            String::new()
        } else {
            format!("  [avp: {}]", case.avp.join(", "))
        };
        println!("  {:<6}{red_text:<20}{green}{tag}", id.to_string());
    }
}

pub fn short(commit: &str) -> &str {
    &commit[..commit.len().min(7)]
}
