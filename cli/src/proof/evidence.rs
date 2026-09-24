//! A spec's evidence/: what is committed, what stays local, and how a run replaces its half of it.
//!
//! Committed evidence is small and meaningful: Assay verdicts and the artifacts a test chose to save (a screenshot,
//! an HTTP log). The runner's full reports and output are regenerable and large (tens of KB per spec, rewritten on
//! every run), so they go to `evidence/raw/`, which `.specs/.gitignore` keeps out of git through a rule the engine
//! writes itself; the receipt keeps their summary and hash. A committed file over [`MAX_COMMITTED_BYTES`] is refused
//! unless gitignored. A file whose content did not meaningfully change (an Assay verdict differing only in its
//! timings) keeps its committed bytes, so re-proving an unchanged spec leaves git clean.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use anyhow::{Context, Result};
use regex::Regex;
use serde_json::Value;

use super::git::Repo;
use super::hash::{self, Hashes};
use super::receipt::{RawReport, Receipt, Report};
use super::report::{self, fm_ids};
use super::scrub::Scrub;
use super::spec::{EVIDENCE_DIR, SPECS_DIR, SpecDir};
use super::summary;

/// The local, gitignored folder inside evidence/ for reports, logs, and anything a test saves under
/// `$SKIES_EVIDENCE/raw/`.
pub const RAW_DIR: &str = "raw";

/// The largest file committed as evidence. Past it, a file belongs under raw/ or in `.specs/.gitignore`.
pub const MAX_COMMITTED_BYTES: u64 = 256 * 1024;

/// The `.specs/.gitignore` rule that keeps every spec's evidence/raw/ out of git.
pub const IGNORE_RULE: &str = "/*/evidence/raw/";

/// The half of evidence/ a run owns: red's files are named `red.*` (in evidence/ and in raw/), green's are the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Half {
    Red,
    Green,
}

impl Half {
    /// Whether a path relative to evidence/ (`red.trx`, `raw/green.log`, `avp-FM-2.json`) belongs to this half.
    pub fn owns(self, rel: &str) -> bool {
        let name = rel.strip_prefix(&format!("{RAW_DIR}/")).unwrap_or(rel);
        name.starts_with("red.") == (self == Half::Red)
    }
}

/// What a run publishes into evidence/.
#[derive(Default)]
pub struct Files<'a> {
    /// The runner's artifacts (`$SKIES_EVIDENCE`), copied as they are; whatever is under raw/ there stays local.
    pub staged: Option<&'a Path>,
    /// Files committed as evidence: `(source, name under evidence/)`.
    pub committed: Vec<(PathBuf, String)>,
    /// Reports and logs kept locally: `(source, name under evidence/raw/)`, scrubbed of machine paths.
    pub raw: Vec<(PathBuf, String)>,
}

/// Makes sure `.specs/.gitignore` keeps evidence/raw/ out of git, adding the rule when it is missing.
pub fn ensure_ignored(root: &Path) -> Result<()> {
    let dir = root.join(SPECS_DIR);
    let path = dir.join(".gitignore");
    let current = std::fs::read_to_string(&path).unwrap_or_default();
    if current.lines().any(|line| line.trim() == IGNORE_RULE) {
        return Ok(());
    }
    let mut text = current;
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str("# Written by `skies proof`: full reports and logs are regenerable and stay local.\n");
    text.push_str(IGNORE_RULE);
    text.push('\n');
    std::fs::create_dir_all(&dir)?;
    std::fs::write(&path, text).with_context(|| format!("writing {}", path.display()))?;
    println!("note: added `{IGNORE_RULE}` to {SPECS_DIR}/.gitignore (reports and logs stay local; commit it)");
    Ok(())
}

/// Why the files would bloat git, if they would: every one to be committed over the cap that git does not ignore.
pub fn oversized(repo: &Repo, spec: &SpecDir, files: &Files) -> Result<Option<String>> {
    let mut large: Vec<(String, u64)> = Vec::new();
    let mut consider = |rel: String, source: &Path| {
        let size = std::fs::metadata(source).map(|meta| meta.len()).unwrap_or(0);
        if size > MAX_COMMITTED_BYTES && !rel.starts_with(&format!("{RAW_DIR}/")) {
            large.push((rel, size));
        }
    };
    if let Some(staged) = files.staged.filter(|staged| staged.is_dir()) {
        for (rel, source) in walk(staged)? {
            consider(rel, &source);
        }
    }
    for (source, name) in &files.committed {
        consider(name.clone(), source);
    }
    if large.is_empty() {
        return Ok(None);
    }
    let paths: Vec<String> = large
        .iter()
        .map(|(rel, _)| format!("{}/{EVIDENCE_DIR}/{rel}", spec.rel()))
        .collect();
    let ignored = repo.ignored(&paths)?;
    let problems: Vec<String> = large
        .iter()
        .zip(&paths)
        .filter(|(_, path)| !ignored.contains(*path))
        .map(|((rel, size), _)| format!("{EVIDENCE_DIR}/{rel} is {} KB", size / 1024))
        .collect();
    if problems.is_empty() {
        return Ok(None);
    }
    Ok(Some(format!(
        "committed evidence is capped at {} KB per file: {}. Keep it local: have the test save it under \
         $SKIES_EVIDENCE/{RAW_DIR}/ (gitignored), or ignore it in {SPECS_DIR}/.gitignore (e.g. `/*/{EVIDENCE_DIR}/<name>`).",
        MAX_COMMITTED_BYTES / 1024,
        problems.join(", ")
    )))
}

/// Replaces one half of evidence/ with `files`. A committed file whose new content is equivalent to what is there
/// (the same bytes, or the same JSON apart from timings) keeps its bytes.
pub fn publish(spec: &SpecDir, half: Half, files: &Files, scrub: &Scrub) -> Result<()> {
    let dir = spec.file(EVIDENCE_DIR);
    let mut previous: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    if dir.is_dir() {
        for (rel, path) in walk(&dir)? {
            if !half.owns(&rel) {
                continue;
            }
            if !rel.starts_with(&format!("{RAW_DIR}/")) {
                previous.insert(rel, std::fs::read(&path)?);
            }
            std::fs::remove_file(&path).with_context(|| format!("removing {}", path.display()))?;
        }
        prune_empty(&dir)?;
    }
    std::fs::create_dir_all(&dir)?;
    let write = |rel: &str, bytes: Vec<u8>| -> Result<()> {
        let target = dir.join(rel);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = match previous.get(rel) {
            Some(old) if !rel.starts_with(&format!("{RAW_DIR}/")) && equivalent(rel, old, &bytes) => old.clone(),
            _ => bytes,
        };
        std::fs::write(&target, bytes).with_context(|| format!("writing {}", target.display()))
    };
    if let Some(staged) = files.staged.filter(|staged| staged.is_dir()) {
        for (rel, source) in walk(staged)? {
            write(&rel, std::fs::read(&source)?)?;
        }
    }
    for (source, name) in &files.committed {
        write(name, std::fs::read(source)?)?;
    }
    if !files.raw.is_empty() {
        std::fs::create_dir_all(dir.join(RAW_DIR))?;
    }
    for (source, name) in &files.raw {
        scrub
            .copy(source, &dir.join(RAW_DIR).join(name))
            .with_context(|| format!("keeping {name} in {EVIDENCE_DIR}/{RAW_DIR}"))?;
    }
    Ok(())
}

/// Copies files into evidence/raw/ alone, touching nothing committed: what `proof run` leaves for inspection.
pub fn keep_local(spec: &SpecDir, files: &[(PathBuf, String)], scrub: &Scrub) -> Result<PathBuf> {
    let raw = spec.file(EVIDENCE_DIR).join(RAW_DIR);
    std::fs::create_dir_all(&raw)?;
    for (source, name) in files {
        scrub
            .copy(source, &raw.join(name))
            .with_context(|| format!("keeping {name} in {EVIDENCE_DIR}/{RAW_DIR}"))?;
    }
    Ok(raw)
}

/// Where a report kept under raw/ is, and its fingerprint, for the receipt.
pub fn raw_report(spec: &SpecDir, name: &str) -> Report {
    let file = format!("{EVIDENCE_DIR}/{RAW_DIR}/{name}");
    let hash = match std::fs::read(spec.path.join(&file)) {
        Ok(bytes) => fingerprint(&String::from_utf8_lossy(&bytes)),
        Err(_) => hash::ABSENT.to_string(),
    };
    Report::Raw(RawReport { file, hash })
}

/// blake3 of a report with its timings blanked (TRX `start`/`duration`/…, JUnit `time`/`timestamp`): the report
/// identifies the run by what it says, not by when it ran or how long it took, so re-proving an unchanged spec
/// writes the same receipt byte for byte while any change in cases, outcomes, or messages changes the hash.
pub fn fingerprint(report: &str) -> String {
    let untimed = TIMINGS.replace_all(report, "${1}=\"\"");
    format!("blake3:{}", blake3::hash(untimed.as_bytes()).to_hex())
}

static TIMINGS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\b(time|timestamp|duration|startTime|endTime|creation|queuing|start|finish)="[^"]*""#)
        .expect("valid regex")
});

/// Hashes the committed evidence as it is now. The half a run did not rewrite (`kept`) keeps the hashes recorded
/// for it: carrying files over without rerunning them must never launder an edit made to them since. `dropped` are
/// recorded paths that no longer count as committed evidence (a report moved to raw/).
pub fn hashes(spec: &SpecDir, previous: Option<&Hashes>, kept: Option<Half>, dropped: &[String]) -> Result<Hashes> {
    let mut hashes = hash::evidence(spec)?;
    let (Some(previous), Some(kept)) = (previous, kept) else {
        return Ok(hashes);
    };
    let in_evidence = |path: &str| path.strip_prefix(&format!("{EVIDENCE_DIR}/")).map(String::from);
    hashes.retain(|path, _| !in_evidence(path).is_some_and(|rel| kept.owns(&rel)));
    for (path, recorded) in previous {
        if in_evidence(path).is_some_and(|rel| kept.owns(&rel)) && !dropped.contains(path) {
            hashes.insert(path.clone(), recorded.clone());
        }
    }
    Ok(hashes)
}

/// Moves the red report a receipt from before compact evidence committed (`evidence/red.trx`, or `red.log` when red
/// did not build) into raw/, summarizing it into the receipt's red cases first. Red is never rerun by `verify`, so
/// this is the only way its cases and messages reach the new shape. Returns the recorded path it moved.
pub fn migrate_red(spec: &SpecDir, receipt: &mut Receipt, scrub: &Scrub) -> Result<Option<String>> {
    let Report::Committed(recorded) = &receipt.red.report else {
        return Ok(None);
    };
    let recorded = recorded.clone();
    let source = spec.path.join(&recorded);
    let name = Path::new(&recorded)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "red.report".to_string());
    if source.is_file() {
        if name.ends_with(".log") {
            receipt.red.output = summary::output(&source, scrub);
        } else if let Ok(parsed) = report::parse(&std::fs::read_to_string(&source)?) {
            for (id, entry) in receipt.red.cases.iter_mut() {
                let cases: Vec<_> = parsed
                    .cases
                    .iter()
                    .filter(|case| fm_ids(&case.name).contains(id))
                    .cloned()
                    .collect();
                entry.cases = summary::names(&cases);
                entry.message = summary::first_failure(&cases, scrub);
            }
        }
        let raw = spec.file(EVIDENCE_DIR).join(RAW_DIR);
        std::fs::create_dir_all(&raw)?;
        std::fs::rename(&source, raw.join(&name)).with_context(|| format!("moving {recorded} to raw/"))?;
    }
    receipt.red.report = raw_report(spec, &name);
    Ok(Some(recorded))
}

/// Whether two versions of a committed file say the same thing: the same bytes, or for JSON (an Assay verdict) the
/// same document once timings are left out.
fn equivalent(rel: &str, old: &[u8], new: &[u8]) -> bool {
    if old == new {
        return true;
    }
    if !rel.ends_with(".json") {
        return false;
    }
    let parse = |bytes: &[u8]| {
        let text = String::from_utf8_lossy(bytes);
        serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')).ok()
    };
    match (parse(old), parse(new)) {
        (Some(mut old), Some(mut new)) => {
            untimed(&mut old);
            untimed(&mut new);
            old == new
        }
        _ => false,
    }
}

/// Drops every key that holds a timing (`durationMs`, `elapsed`, `timestamp`, `startedAt`), at any depth.
fn untimed(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.retain(|key, _| {
                let key = key.to_ascii_lowercase();
                let timing = ["duration", "elapsed", "timestamp"]
                    .iter()
                    .any(|part| key.contains(part))
                    || ["time", "startedat", "finishedat"].contains(&key.as_str());
                !timing
            });
            map.values_mut().for_each(untimed);
        }
        Value::Array(items) => items.iter_mut().for_each(untimed),
        _ => {}
    }
}

/// Every file under `dir`, as `(path relative to dir with forward slashes, absolute path)`, sorted.
fn walk(dir: &Path) -> Result<Vec<(String, PathBuf)>> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(current) = pending.pop() {
        for entry in std::fs::read_dir(&current).with_context(|| format!("reading {}", current.display()))? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                pending.push(entry.path());
            } else {
                found.push((hash::relative(dir, &entry.path()), entry.path()));
            }
        }
    }
    found.sort();
    Ok(found)
}

/// Removes folders left empty under `dir` (not `dir` itself), deepest first.
fn prune_empty(dir: &Path) -> Result<()> {
    let mut folders: BTreeSet<PathBuf> = BTreeSet::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(current) = pending.pop() {
        for entry in std::fs::read_dir(&current)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                folders.insert(entry.path());
                pending.push(entry.path());
            }
        }
    }
    for folder in folders.iter().rev() {
        let _ = std::fs::remove_dir(folder);
    }
    Ok(())
}

#[cfg(test)]
#[path = "evidence_tests.rs"]
mod tests;
