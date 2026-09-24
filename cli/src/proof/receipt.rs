//! `receipt.json`: the record that a spec's failure modes failed before the change and pass after it.
//!
//! It is a record, not a turnstile: nothing blocks on it. Keys are written in a fixed order (struct order, then
//! sorted maps) and nothing in it varies between two runs of the same code (no timestamps, no durations), so
//! re-verifying an unchanged spec rewrites nothing and a re-recorded receipt diffs only where the proof moved.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::footprint::{self, Source};
use super::hash::{self, Hashes};
use super::lines::{self, Prints, Snapshot};
use super::report::FmId;
use super::spec::{RECEIPT_FILE, SpecDir, SpecDoc};
use crate::manifest::Project;

#[derive(Debug, Serialize, Deserialize)]
pub struct Receipt {
    pub spec: String,
    pub runner: String,
    pub red: Red,
    pub green: Green,
    /// Source files the receipt depends on, relative to the project root. A file the green run executed is pinned by
    /// its executed lines, one hash per run of them (`{"lines": "12-18,40", "ranges": "9f…,03…"}`), any other by its
    /// whole-file hash. If an executed run's text changes or is no longer found in order, or a whole file changes,
    /// the receipt is stale.
    pub footprint: Prints,
    /// `coverage` when the footprint is the files the green run executed (plus the diff and `touches`), `diff` when
    /// it is only the files changed since red plus `touches`. Receipts written before coverage read as `diff`.
    #[serde(default)]
    pub footprint_source: Source,
    /// With a coverage footprint, the files changed between red and green. `verify` refreshes the executed files
    /// from its own run and keeps these, since the change itself is what the receipt proves.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub footprint_changed: Vec<String>,
    /// What the receipt was recorded from: spec.md, the e2e files, and the root lockfiles.
    pub inputs: Hashes,
    /// Every committed file under evidence/ (evidence/raw/ is local and left out), relative to the spec folder. The
    /// evidence is the frozen artifact a reviewer reads, so an edit to it after recording is tampering, not
    /// staleness. Absent on receipts written before evidence was hashed, which are then not checked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<Hashes>,
    /// The module ctx.md files revised in the same change (red..working tree), relative to the project root: the
    /// record that the prose explaining the touched modules was revisited with this proof.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ctx_revised: Vec<String>,
    /// Other specs whose footprint overlaps this one's and that `record --with-impacted` re-proved green in the same
    /// run: spec folder → the blake3 of that spec's receipt right after its verify.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub verified_with: BTreeMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Red {
    pub commit: String,
    /// Present when red is `commit` plus a patch that removes the feature (specs written after the code).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch: Option<Patch>,
    pub cases: BTreeMap<FmId, Entry<RedCase>>,
    /// The runner's full report (or, when red did not build, its output), kept locally under evidence/raw/.
    pub report: Report,
    /// When red did not build: the lines of the runner's output that say why, so the receipt explains itself
    /// without the local log.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Patch {
    /// Relative to the spec folder.
    pub file: String,
    pub hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RedCase {
    Fail,
    /// The E2E did not build on red (it references code the feature adds), so no case could pass.
    DidNotBuild,
    /// Passed on red: the case does not bite. Allowed only with a justification in spec.md.
    NonDiscriminating,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Green {
    pub commit: String,
    /// Whether the working tree had uncommitted changes outside the spec folder when green ran.
    pub dirty: bool,
    pub cases: BTreeMap<FmId, Entry<GreenCase>>,
    /// The runner's full report, kept locally under evidence/raw/.
    pub report: Report,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GreenCase {
    Pass,
}

/// Where a run's full report is. The report is regenerable and never committed: the receipt keeps what matters
/// from it (per mode, the result, the cases, and on red what failed) and its hash, so a report found on disk can be
/// matched to the run it came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Report {
    Raw(RawReport),
    /// A receipt written before compact evidence: the path of a report committed under evidence/. Still read;
    /// `verify` and `record` move the file to evidence/raw/ and rewrite this as [`Report::Raw`].
    Committed(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawReport {
    /// Relative to the spec folder, under evidence/raw/ (gitignored).
    pub file: String,
    /// blake3 of what the report says (each case's name, outcome, and message, sorted), so the same results hash
    /// the same whatever the timings or the order the runner wrote them in.
    pub hash: String,
}

/// One failure mode's outcome in a run: the result, the cases that decided it, and on red the first thing a failing
/// case said. A mode tagged `[avp: …]` also names its criteria and the verdict file. No durations or timestamps: a
/// receipt of an unchanged run must be byte for byte the same, and timings live in the local report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "EntryShape<T>", bound(deserialize = "T: Deserialize<'de>"))]
pub struct Entry<T> {
    pub result: T,
    /// The criterion ids from the spec.md tag.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub avp: Vec<String>,
    /// The verdict saved by the case, relative to the spec folder. Always present on a tagged green; on red only
    /// when the case got far enough to write one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verdict: Option<String>,
    /// The names of the cases naming this mode, sorted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cases: Vec<String>,
    /// On red, the start of what the first failing case reported, trimmed to a few lines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// What an entry may look like on disk: the full object, or a bare `"pass"` as receipts before compact evidence
/// wrote an untagged mode.
#[derive(Deserialize)]
#[serde(untagged)]
enum EntryShape<T> {
    Bare(T),
    Full {
        result: T,
        #[serde(default)]
        avp: Vec<String>,
        #[serde(default)]
        verdict: Option<String>,
        #[serde(default)]
        cases: Vec<String>,
        #[serde(default)]
        message: Option<String>,
    },
}

impl<T> From<EntryShape<T>> for Entry<T> {
    fn from(shape: EntryShape<T>) -> Entry<T> {
        match shape {
            EntryShape::Bare(result) => Entry {
                result,
                avp: Vec::new(),
                verdict: None,
                cases: Vec::new(),
                message: None,
            },
            EntryShape::Full {
                result,
                avp,
                verdict,
                cases,
                message,
            } => Entry {
                result,
                avp,
                verdict,
                cases,
                message,
            },
        }
    }
}

impl<T: Copy> Entry<T> {
    /// A mode's entry; `verdict` is kept only for a tagged mode.
    pub fn new(result: T, avp: &[String], verdict: Option<String>) -> Entry<T> {
        Entry {
            result,
            avp: avp.to_vec(),
            verdict: verdict.filter(|_| !avp.is_empty()),
            cases: Vec::new(),
            message: None,
        }
    }

    /// The same entry, naming the cases that decided it and what the first failing one said.
    pub fn with_cases(mut self, cases: Vec<String>, message: Option<String>) -> Entry<T> {
        self.cases = cases;
        self.message = message;
        self
    }

    pub fn result(&self) -> T {
        self.result
    }
}

impl Receipt {
    pub fn load(spec: &SpecDir) -> Result<Option<Receipt>> {
        let path = spec.file(RECEIPT_FILE);
        if !path.is_file() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let receipt = serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        Ok(Some(receipt))
    }

    /// Writes the receipt, leaving the file alone when its bytes would not change, so an unchanged proof never
    /// touches the file (nor its modification time).
    pub fn save(&self, spec: &SpecDir) -> Result<()> {
        let path = spec.file(RECEIPT_FILE);
        let text = self.to_text()?;
        if std::fs::read_to_string(&path).is_ok_and(|current| current == text) {
            return Ok(());
        }
        std::fs::write(&path, text).with_context(|| format!("writing {}", path.display()))
    }

    pub fn to_text(&self) -> Result<String> {
        let mut text = serde_json::to_string_pretty(self)?;
        text.push('\n');
        Ok(text)
    }

    /// Whether the receipt still points at reports committed under evidence/ (written before compact evidence).
    pub fn has_committed_reports(&self) -> bool {
        matches!(self.red.report, Report::Committed(_)) || matches!(self.green.report, Report::Committed(_))
    }
}

/// Where a receipt stands against the working tree right now.
pub enum Freshness {
    Missing,
    Current,
    Stale(Vec<String>),
    /// Files under evidence/ differ from what the receipt recorded. Reported before staleness, because a receipt
    /// whose evidence was edited no longer describes any run at all.
    Tampered(Vec<String>),
}

/// Rehashes the evidence, the recorded footprint, and today's inputs (spec.md, e2e files, lockfiles, and anything new
/// that matches `touches` within the runner's scope). Filesystem only, so it stays in milliseconds.
pub fn freshness(project: &Project, spec: &SpecDir) -> Result<Freshness> {
    freshness_with(project, spec, Receipt::load(spec)?, &Snapshot::default())
}

/// [`freshness`] for an already loaded receipt, reading today's files from `known` where it has them. Coverage
/// footprints of sibling specs share most of their files (the host wiring, the entities), so `status` reads each
/// file once.
pub fn freshness_with(
    project: &Project,
    spec: &SpecDir,
    receipt: Option<Receipt>,
    known: &Snapshot,
) -> Result<Freshness> {
    let Some(receipt) = receipt else {
        return Ok(Freshness::Missing);
    };
    if let Some(recorded) = &receipt.evidence {
        let tampered = hash::changed(recorded, &hash::evidence(spec)?);
        if !tampered.is_empty() {
            return Ok(Freshness::Tampered(tampered));
        }
    }
    let root = project.root.as_path();
    let doc = SpecDoc::load(spec)?;
    let touched = footprint::touched(root, &doc, footprint::runner_of(project, &doc))?;
    let inputs = hash::input_paths(root, spec)?;

    let mut changed = footprint_changed(root, &receipt.footprint, &touched, known);
    changed.extend(hash::changed(&receipt.inputs, &hash::hash_all(root, &inputs)));
    Ok(if changed.is_empty() {
        Freshness::Current
    } else {
        Freshness::Stale(changed)
    })
}

/// The footprint files that no longer match their print, plus files matching `touches` today that the receipt never
/// recorded. Each file is read once, from `known` when it holds it.
fn footprint_changed(root: &Path, footprint: &Prints, touched: &BTreeSet<String>, known: &Snapshot) -> Vec<String> {
    let unread = footprint.keys().filter(|path| !known.contains(path));
    let local = Snapshot::read(root, unread);
    let mut changed: Vec<String> = footprint
        .iter()
        .filter(|(path, print)| {
            let content = known.get(path).or_else(|| local.get(path)).flatten();
            !lines::matches(print, content)
        })
        .map(|(path, _)| path.clone())
        .collect();
    changed.extend(touched.iter().filter(|path| !footprint.contains_key(*path)).cloned());
    changed.sort();
    changed
}

#[cfg(test)]
#[path = "receipt_tests.rs"]
mod tests;
