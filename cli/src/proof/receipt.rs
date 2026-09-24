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
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Red {
    pub commit: String,
    /// Present when red is `commit` plus a patch that removes the feature (specs written after the code).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch: Option<Patch>,
    pub cases: BTreeMap<FmId, RedCase>,
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
    /// Passed on red: the case does not bite. Allowed only with a justification in spec.md.
    NonDiscriminating,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Green {
    pub commit: String,
    /// Whether the working tree had uncommitted changes outside the spec folder when green ran.
    pub dirty: bool,
    pub cases: BTreeMap<FmId, GreenCase>,
    /// The green report, relative to the spec folder.
    pub report: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GreenCase {
    Pass,
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
}

/// Rehashes the recorded footprint and today's inputs (spec.md, e2e files, lockfiles, and anything new that matches
/// `touches`). Filesystem only, so it stays in milliseconds.
pub fn freshness(root: &Path, spec: &SpecDir) -> Result<Freshness> {
    let Some(receipt) = Receipt::load(spec)? else {
        return Ok(Freshness::Missing);
    };
    let doc = SpecDoc::load(spec)?;
    let mut footprint_paths: Vec<&String> = receipt.footprint.keys().collect();
    let touched = hash::touched_paths(root, &doc.touches)?;
    footprint_paths.extend(touched.iter().filter(|path| !receipt.footprint.contains_key(*path)));
    let inputs = hash::input_paths(root, spec)?;

    let mut changed = hash::changed(&receipt.footprint, &hash::hash_all(root, footprint_paths));
    changed.extend(hash::changed(&receipt.inputs, &hash::hash_all(root, &inputs)));
    Ok(if changed.is_empty() { Freshness::Current } else { Freshness::Stale(changed) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_in_a_stable_readable_shape() {
        let receipt = Receipt {
            spec: "0001-a".into(),
            runner: "api".into(),
            red: Red {
                commit: "abc".into(),
                patch: None,
                cases: [(FmId(2), RedCase::NonDiscriminating), (FmId(1), RedCase::Fail)].into(),
                report: "evidence/red.xml".into(),
            },
            green: Green {
                commit: "def".into(),
                dirty: false,
                cases: [(FmId(1), GreenCase::Pass), (FmId(2), GreenCase::Pass)].into(),
                report: "evidence/green.xml".into(),
            },
            footprint: [("src/A.cs".into(), "blake3:00".into())].into(),
            inputs: Hashes::new(),
        };
        let json = serde_json::to_string(&receipt).unwrap();
        assert_eq!(
            json,
            r#"{"spec":"0001-a","runner":"api","red":{"commit":"abc","cases":{"FM-1":"fail","FM-2":"non-discriminating"},"report":"evidence/red.xml"},"green":{"commit":"def","dirty":false,"cases":{"FM-1":"pass","FM-2":"pass"},"report":"evidence/green.xml"},"footprint":{"src/A.cs":"blake3:00"},"inputs":{}}"#
        );
        let back: Receipt = serde_json::from_str(&json).unwrap();
        assert_eq!(back.red.cases[&FmId(2)], RedCase::NonDiscriminating);
    }
}
