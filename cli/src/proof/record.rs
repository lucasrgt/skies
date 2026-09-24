//! `skies proof record`: red in a throwaway worktree, green in the working tree, then the receipt.
//!
//! Red answers "do these cases bite?": every failure mode must fail where the feature does not exist yet. Green
//! answers "does the feature handle them?". Only when both hold is a receipt written, so a receipt on disk always
//! describes a complete red→green pair.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use super::git::Repo;
use super::green::{self, GreenOutcome, join};
use super::hash;
use super::receipt::{Entry, Green, Patch, Receipt, Red, RedCase};
use super::report::{self, FmId, Report};
use super::runner::{Job, NoReport, Session};
use super::scrub::Scrub;
use super::spec::{self, E2E_DIR, EVIDENCE_DIR, RED_PATCH_FILE, SPEC_FILE, SpecDir, SpecDoc};
use super::{avp, ctx, footprint, impact, lines, verify};
use crate::manifest::Project;

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
    let red_commit = match (options.red, &patch) {
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

    // Red, in a worktree that is gone again before green starts. Its `{evidence}` lives in scratch, so an Assay
    // verdict a red case saved survives the worktree.
    let red_evidence = scratch.path().join("red-evidence");
    let (red_run, scrub) = {
        let worktree = repo.temp_worktree(&red_commit)?;
        // Built while the worktree exists, so its canonical path can be resolved: red and green paths both become
        // `{root}` in the evidence.
        let scrub = Scrub::new(&[&repo.top, &worktree.path]);
        let red_root = worktree.path.join(&repo.prefix);
        let red_spec = SpecDir {
            path: red_root.join(spec.rel()),
            ..spec.clone()
        };
        copy_spec_sources(&spec, &red_spec)?;
        if let Some(patch) = &patch {
            repo.apply(&worktree.path, patch)?;
        }
        let run = session.run(&Job {
            runner_name,
            runner,
            spec: &red_spec,
            root: &red_root,
            evidence: &red_evidence,
            scratch: scratch.path(),
            label: "red",
        });
        // A configured report path lives inside the worktree; keep the report (or the build log) past its removal.
        let red_run = match run {
            Ok(run) => {
                let kept = scratch.path().join("red.report");
                std::fs::copy(&run.file, &kept)?;
                RedRun::Report(run.report, kept)
            }
            Err(error) => match error.downcast_ref::<NoReport>() {
                Some(no_report) => {
                    let kept = scratch.path().join("red.build.log");
                    std::fs::copy(&no_report.log, &kept)?;
                    RedRun::DidNotBuild(kept)
                }
                None => return Err(error),
            },
        };
        (red_run, scrub)
    };
    let (red_cases, red_file, red_report): (BTreeMap<FmId, Entry<RedCase>>, PathBuf, String) = match red_run {
        RedRun::DidNotBuild(log) => {
            println!(
                "  red did not build at {}: every failure mode counts as failing (build output in {EVIDENCE_DIR}/red.log)",
                short(&red_commit)
            );
            let cases = doc
                .failure_modes
                .iter()
                .map(|id| (*id, Entry::new(RedCase::DidNotBuild, doc.avp(*id), None)))
                .collect();
            (cases, log, format!("{EVIDENCE_DIR}/red.log"))
        }
        RedRun::Report(report, file) => {
            let red_eval = match report::evaluate(&doc.failure_modes, &report.cases) {
                Ok(evaluation) => evaluation,
                Err(problems) => {
                    eprintln!("{}: the red run does not match spec.md:\n{problems}", spec.name);
                    return Ok(1);
                }
            };
            let cases = red_eval
                .passed
                .iter()
                .map(|(id, passed)| (*id, red_entry(&doc, *id, *passed, &red_evidence)))
                .collect();
            (cases, file, format!("{EVIDENCE_DIR}/red.{}", report.format.extension()))
        }
    };
    let unjustified: Vec<FmId> = red_cases
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
    print_cases(&red_cases);

    let mut files = vec![
        (red_file, red_report.clone()),
        (proven.file.clone(), proven.report.clone()),
    ];
    for id in &doc.failure_modes {
        let name = avp::verdict_file(*id);
        if !doc.avp(*id).is_empty() && red_evidence.join(&name).is_file() {
            files.push((red_evidence.join(&name), format!("{EVIDENCE_DIR}/red.{name}")));
        }
    }
    green::publish_evidence(&spec, &proven.staged, &files, false, &scrub)?;

    let changed = match patch.as_deref() {
        Some(patch) => repo.patch_footprint(patch)?,
        None => repo.changed_since(&red_commit)?,
    };
    let footprint = footprint::build(root, &doc, &changed, &proven)?;
    let prints = lines::print_all(root, &footprint.paths, &footprint.executed);
    println!(
        "  {}",
        footprint::describe(&footprint, &proven, lines::by_lines(&prints))
    );
    let footprint_paths = footprint.paths;
    let ctx_revised = revised_ctx(&repo, &changed, patch.is_some().then_some(head.as_str()))?;
    let mut receipt = Receipt {
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
    impacted(&project, &spec, &mut receipt, options.with_impacted)
}

/// A spec that touched a module whose ctx.md stayed as it was gets a note, never a failure: the change may well
/// have left every invariant intact, and only the author can tell.
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

/// The ctx files revised in this change: those in the red..working-tree diff and, when red is HEAD plus a
/// red.patch (which removes the feature and rarely touches prose), those edited in the working tree since HEAD.
fn revised_ctx(repo: &Repo, changed: &[String], patched_head: Option<&str>) -> Result<Vec<String>> {
    let mut revised: BTreeSet<String> = changed.iter().filter(|path| ctx::is_ctx(path)).cloned().collect();
    if let Some(head) = patched_head {
        revised.extend(repo.changed_since(head)?.into_iter().filter(|path| ctx::is_ctx(path)));
    }
    Ok(revised.into_iter().collect())
}

/// Lists the other specs this receipt's footprint reaches and, with `--with-impacted`, re-proves them green. The
/// new receipt is already written; `verified_with` is added only for specs that passed, and any failure makes the
/// exit code 1 so the author sees that the change broke a neighbor.
fn impacted(project: &Project, spec: &SpecDir, receipt: &mut Receipt, rerun: bool) -> Result<u8> {
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
    let outcomes = verify::verify_specs(project, &specs)?;
    verify::print_outcomes(&outcomes);
    receipt.verified_with = outcomes
        .iter()
        .filter_map(|outcome| Some((outcome.name.clone(), outcome.result.as_ref().ok()?.receipt.clone())))
        .collect();
    receipt.save(spec)?;
    let failed: Vec<&str> = outcomes
        .iter()
        .filter(|outcome| outcome.result.is_err())
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

/// A failure mode passes red, and so bites nothing, only as green would count it passing: every case passed and,
/// for a tagged mode, the verdict passed too. A missing verdict on red is simply a failure.
fn red_entry(doc: &SpecDoc, id: FmId, cases_passed: bool, evidence: &Path) -> Entry<RedCase> {
    let result = if cases_passed && avp::check(doc, id, evidence).is_ok() {
        RedCase::NonDiscriminating
    } else {
        RedCase::Fail
    };
    let name = avp::verdict_file(id);
    let verdict = evidence
        .join(&name)
        .is_file()
        .then(|| format!("{EVIDENCE_DIR}/red.{name}"));
    Entry::new(result, doc.avp(id), verdict)
}

/// What the red revision produced: a report to evaluate, or a build that never got that far.
enum RedRun {
    Report(Report, PathBuf),
    DidNotBuild(PathBuf),
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
        green::copy_dir(&from.file(E2E_DIR), &e2e)?;
    }
    Ok(())
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
