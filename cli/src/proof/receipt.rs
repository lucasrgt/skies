//! `receipt.json`: the record that a spec's failure modes failed before the change and pass after it.
//!
//! It is a record, not a turnstile: nothing blocks on it. Keys are written in a fixed order (struct order, then
//! sorted maps) so a re-recorded receipt diffs cleanly in review.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::hash::{self, Hashes};
use super::report::FmId;
use super::spec::{RECEIPT_FILE, SpecDir, SpecDoc};

#[derive(Debug, Serialize, Deserialize)]
pub struct Receipt {
    pub spec: String,
    pub runner: String,
    pub red: Red,
    pub green: Green,
    /// Source files the change covers, relative to the project root. If any changes, the receipt is stale.
    pub footprint: Hashes,
    /// What the receipt was recorded from: spec.md, the e2e files, and the root lockfiles.
    pub inputs: Hashes,
    /// Every committed file under evidence/, relative to the spec folder. The evidence is the frozen artifact a
    /// reviewer replays or reads, so an edit to it after recording is tampering, not staleness. Absent on receipts
    /// written before evidence was hashed, which are then not checked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<Hashes>,
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
    /// The red report, relative to the spec folder.
    pub report: String,
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
    /// The green report, relative to the spec folder.
    pub report: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GreenCase {
    Pass,
}

/// One failure mode's outcome in a run. A plain `"pass"` for most; an object naming the Assay criteria and the
/// verdict file for a mode tagged `[avp: …]`, so the reader of the receipt sees which verifier decided it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Entry<T> {
    Plain(T),
    Avp(AvpEntry<T>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AvpEntry<T> {
    pub result: T,
    /// The criterion ids from the spec.md tag.
    pub avp: Vec<String>,
    /// The verdict saved by the case, relative to the spec folder. Always present on green; on red only when the
    /// case got far enough to write one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verdict: Option<String>,
}

impl<T: Copy> Entry<T> {
    /// A plain entry for an untagged mode, the detailed one for a tagged mode.
    pub fn new(result: T, avp: &[String], verdict: Option<String>) -> Entry<T> {
        if avp.is_empty() {
            Entry::Plain(result)
        } else {
            Entry::Avp(AvpEntry {
                result,
                avp: avp.to_vec(),
                verdict,
            })
        }
    }

    pub fn result(&self) -> T {
        match self {
            Entry::Plain(result) => *result,
            Entry::Avp(entry) => entry.result,
        }
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

    pub fn save(&self, spec: &SpecDir) -> Result<()> {
        let path = spec.file(RECEIPT_FILE);
        let mut text = serde_json::to_string_pretty(self)?;
        text.push('\n');
        std::fs::write(&path, text).with_context(|| format!("writing {}", path.display()))
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
/// that matches `touches`). Filesystem only, so it stays in milliseconds.
pub fn freshness(root: &Path, spec: &SpecDir) -> Result<Freshness> {
    let Some(receipt) = Receipt::load(spec)? else {
        return Ok(Freshness::Missing);
    };
    if let Some(recorded) = &receipt.evidence {
        let tampered = hash::changed(recorded, &hash::evidence(spec)?);
        if !tampered.is_empty() {
            return Ok(Freshness::Tampered(tampered));
        }
    }
    let doc = SpecDoc::load(spec)?;
    let mut footprint_paths: Vec<&String> = receipt.footprint.keys().collect();
    let touched = hash::touched_paths(root, &doc.touches)?;
    footprint_paths.extend(touched.iter().filter(|path| !receipt.footprint.contains_key(*path)));
    let inputs = hash::input_paths(root, spec)?;

    let mut changed = hash::changed(&receipt.footprint, &hash::hash_all(root, footprint_paths));
    changed.extend(hash::changed(&receipt.inputs, &hash::hash_all(root, &inputs)));
    Ok(if changed.is_empty() {
        Freshness::Current
    } else {
        Freshness::Stale(changed)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_in_a_stable_readable_shape() {
        let avp = ["idempotency-key-honored".to_string()];
        let receipt = Receipt {
            spec: "0001-a".into(),
            runner: "api".into(),
            red: Red {
                commit: "abc".into(),
                patch: None,
                cases: [
                    (FmId(2), Entry::new(RedCase::NonDiscriminating, &[], None)),
                    (FmId(1), Entry::new(RedCase::Fail, &[], None)),
                    (FmId(3), Entry::new(RedCase::Fail, &avp, None)),
                ]
                .into(),
                report: "evidence/red.xml".into(),
            },
            green: Green {
                commit: "def".into(),
                dirty: false,
                cases: [
                    (FmId(1), Entry::new(GreenCase::Pass, &[], None)),
                    (FmId(2), Entry::new(GreenCase::Pass, &[], None)),
                    (
                        FmId(3),
                        Entry::new(GreenCase::Pass, &avp, Some("evidence/avp-FM-3.json".into())),
                    ),
                ]
                .into(),
                report: "evidence/green.xml".into(),
            },
            footprint: [("src/A.cs".into(), "blake3:00".into())].into(),
            inputs: Hashes::new(),
            evidence: Some([("evidence/green.xml".into(), "blake3:01".into())].into()),
            verified_with: BTreeMap::new(),
        };
        let json = serde_json::to_string(&receipt).unwrap();
        assert_eq!(
            json,
            r#"{"spec":"0001-a","runner":"api","red":{"commit":"abc","cases":{"FM-1":"fail","FM-2":"non-discriminating","FM-3":{"result":"fail","avp":["idempotency-key-honored"]}},"report":"evidence/red.xml"},"green":{"commit":"def","dirty":false,"cases":{"FM-1":"pass","FM-2":"pass","FM-3":{"result":"pass","avp":["idempotency-key-honored"],"verdict":"evidence/avp-FM-3.json"}},"report":"evidence/green.xml"},"footprint":{"src/A.cs":"blake3:00"},"inputs":{},"evidence":{"evidence/green.xml":"blake3:01"}}"#
        );
        let back: Receipt = serde_json::from_str(&json).unwrap();
        assert_eq!(back.red.cases[&FmId(2)].result(), RedCase::NonDiscriminating);
        assert_eq!(back.green.cases[&FmId(3)].result(), GreenCase::Pass);
        assert!(matches!(&back.green.cases[&FmId(3)], Entry::Avp(entry) if entry.avp == avp));
    }

    #[test]
    fn reads_receipts_written_before_evidence_hashes() {
        let old = r#"{"spec":"0001-a","runner":"api","red":{"commit":"a","cases":{"FM-1":"fail"},"report":"r"},"green":{"commit":"b","dirty":false,"cases":{"FM-1":"pass"},"report":"g"},"footprint":{},"inputs":{}}"#;
        let receipt: Receipt = serde_json::from_str(old).unwrap();
        assert!(receipt.evidence.is_none());
        assert!(receipt.verified_with.is_empty());
    }
}
