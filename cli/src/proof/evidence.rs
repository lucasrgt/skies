//! A spec's evidence/: what is committed and what stays local.
//!
//! Committed evidence is small and meaningful: Assay verdicts and the artifacts a test chose to save under
//! `$SKIES_EVIDENCE` (a screenshot, an HTTP log). The runner's full reports and output are regenerable and large, so
//! they go to `evidence/raw/`, which `.specs/.gitignore` keeps out of git through a rule the engine writes itself. A
//! committed file over [`MAX_COMMITTED_BYTES`] is refused. A file whose content did not meaningfully change (an
//! Assay verdict differing only in its timings) keeps its committed bytes, so re-recording an unchanged spec leaves
//! git clean.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_json::Value;

use super::spec::{EVIDENCE_DIR, SPECS_DIR, SpecDir};

/// The local, gitignored folder inside evidence/ for reports, logs, and anything a test saves under
/// `$SKIES_EVIDENCE/raw/`.
pub const RAW_DIR: &str = "raw";

/// The largest file committed as evidence. Past it, a file belongs under `$SKIES_EVIDENCE/raw/`.
pub const MAX_COMMITTED_BYTES: u64 = 256 * 1024;

/// The `.specs/.gitignore` rule that keeps every spec's evidence/raw/ out of git.
pub const IGNORE_RULE: &str = "/*/evidence/raw/";

/// Makes sure `.specs/.gitignore` keeps evidence/raw/ out of git, adding the rule when it is missing.
pub fn ensure_ignored(root: &Path) -> Result<()> {
    let dir = root.join(SPECS_DIR);
    let path = dir.join(".gitignore");
    let mut text = std::fs::read_to_string(&path).unwrap_or_default();
    if text.lines().any(|line| line.trim() == IGNORE_RULE) {
        return Ok(());
    }
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

/// What a record puts into evidence/: `(source, name under evidence/)` pairs, committed or kept under raw/.
#[derive(Default)]
pub struct Files {
    pub committed: Vec<(PathBuf, String)>,
    pub raw: Vec<(PathBuf, String)>,
}

impl Files {
    /// Adds everything a run saved under `$SKIES_EVIDENCE`: committed, except what it saved under raw/.
    pub fn add_saved(&mut self, staged: &Path) -> Result<()> {
        if !staged.is_dir() {
            return Ok(());
        }
        for (rel, source) in walk(staged)? {
            if rel.starts_with(&format!("{RAW_DIR}/")) {
                self.raw.push((source, rel[RAW_DIR.len() + 1..].to_string()));
            } else {
                self.committed.push((source, rel));
            }
        }
        Ok(())
    }
}

/// Replaces the spec's evidence/ with `files`, after refusing any committed file over the cap. A committed file whose
/// new content is equivalent to what is there (the same bytes, or the same JSON apart from timings) keeps its bytes.
pub fn publish(spec: &SpecDir, files: &Files) -> Result<()> {
    let large: Vec<String> = files
        .committed
        .iter()
        .filter_map(|(source, name)| {
            let size = std::fs::metadata(source).map(|meta| meta.len()).unwrap_or(0);
            (size > MAX_COMMITTED_BYTES).then(|| format!("{EVIDENCE_DIR}/{name} is {} KB", size / 1024))
        })
        .collect();
    if !large.is_empty() {
        bail!(
            "committed evidence is capped at {} KB per file: {}. Keep it local: have the test save it under \
             $SKIES_EVIDENCE/{RAW_DIR}/ (gitignored).",
            MAX_COMMITTED_BYTES / 1024,
            large.join(", ")
        );
    }
    let dir = spec.file(EVIDENCE_DIR);
    let mut previous: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    if dir.is_dir() {
        for (rel, path) in walk(&dir)? {
            if !rel.starts_with(&format!("{RAW_DIR}/")) {
                previous.insert(rel, std::fs::read(&path)?);
            }
        }
        std::fs::remove_dir_all(&dir).with_context(|| format!("clearing {}", dir.display()))?;
    }
    for (source, name) in &files.committed {
        let bytes = std::fs::read(source)?;
        let bytes = match previous.remove(name) {
            Some(old) if equivalent(name, &old, &bytes) => old,
            _ => bytes,
        };
        write(&dir.join(name), &bytes)?;
    }
    keep_local(spec, &files.raw)?;
    Ok(())
}

/// Copies files into evidence/raw/ alone, touching nothing committed: what `proof run` leaves for inspection.
pub fn keep_local(spec: &SpecDir, files: &[(PathBuf, String)]) -> Result<PathBuf> {
    let raw = spec.file(EVIDENCE_DIR).join(RAW_DIR);
    std::fs::create_dir_all(&raw)?;
    for (source, name) in files {
        write(&raw.join(name), &std::fs::read(source)?)?;
    }
    Ok(raw)
}

fn write(target: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(target, bytes).with_context(|| format!("writing {}", target.display()))
}

/// Whether two versions of a committed file say the same thing: the same bytes, or for JSON (an Assay verdict) the
/// same document once timings are left out.
fn equivalent(name: &str, old: &[u8], new: &[u8]) -> bool {
    if old == new {
        return true;
    }
    let parse = |bytes: &[u8]| {
        let text = String::from_utf8_lossy(bytes);
        serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')).ok()
    };
    match (name.ends_with(".json"), parse(old), parse(new)) {
        (true, Some(mut old), Some(mut new)) => {
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
                let rel = entry.path().strip_prefix(dir).unwrap_or(&entry.path()).to_path_buf();
                let rel: Vec<String> = rel
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy().into_owned())
                    .collect();
                found.push((rel.join("/"), entry.path()));
            }
        }
    }
    found.sort();
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_verdict_that_differs_only_in_timings_is_equivalent() {
        let old = br#"{"results":[{"criterionId":"a","status":"pass","durationMs":12}],"startedAt":"x"}"#;
        let new = br#"{"results":[{"criterionId":"a","status":"pass","durationMs":40}],"startedAt":"y"}"#;
        assert!(equivalent("avp-FM-1.json", old, new));
        let failed = br#"{"results":[{"criterionId":"a","status":"fail","durationMs":40}]}"#;
        assert!(!equivalent("avp-FM-1.json", old, failed));
        assert!(!equivalent("log.txt", b"a", b"b"));
    }
}
