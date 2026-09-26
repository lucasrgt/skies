//! `receipt.json`: the record that a spec's failure modes failed before the change and passed after it.
//!
//! It proves red→green once; CI keeps green passing from then on, so nothing here pins files or detects drift. It
//! pins what produced it instead: red's and green's commits (and whether green had uncommitted changes) and the
//! runner's commands. Keys are written in a fixed order (struct order, then sorted maps) and nothing in it varies
//! between two runs of the same code (no timestamps, durations, or report hashes), so re-recording an unchanged spec
//! rewrites nothing.
//!
//! What a receipt proves is narrow and worth saying plainly: the named cases failed on red and passed on green under
//! the recorded runner. It does not prove their assertions are meaningful; review of the cases, and an Assay verdict
//! for a mode tagged `[avp: …]`, is the layer that checks meaning.

use std::collections::BTreeMap;

use anyhow::{Context, Result};
use serde::Serialize;

use super::report::FmId;
use super::spec::{RECEIPT_FILE, SpecDir};
use crate::manifest::Runner;

#[derive(Debug, Serialize)]
pub struct Receipt {
    pub spec: String,
    pub runner: String,
    /// What the runner ran, so a receipt names the commands that produced it and a changed runner is visible.
    pub commands: Commands,
    pub red: Red,
    pub green: Green,
    pub failure_modes: BTreeMap<FmId, Mode>,
}

#[derive(Debug, Serialize)]
pub struct Red {
    pub commit: String,
    /// The patch applied on `commit` to remove the feature (a spec written after the code), relative to the spec.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch: Option<String>,
    /// When red did not build: the lines of the runner's output that say why.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Green {
    pub commit: String,
    /// Set when `--allow-dirty` recorded green from a working tree that differs from `commit`.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub dirty: bool,
}

/// The runner's commands as declared in Skies.toml (placeholders unexpanded, since their values are paths that
/// change every run), and a hash of the three that tells two receipts' runners apart at a glance.
#[derive(Debug, Serialize)]
pub struct Commands {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setup: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build: Option<String>,
    pub command: String,
    /// `blake3:<hex>` of `setup`, `build`, and `command` (an absent one empty), each followed by a NUL.
    pub hash: String,
}

impl Commands {
    pub fn of(runner: &Runner) -> Commands {
        let mut hasher = blake3::Hasher::new();
        for part in [
            runner.setup.as_deref(),
            runner.build.as_deref(),
            Some(runner.command.as_str()),
        ] {
            hasher.update(part.unwrap_or_default().as_bytes());
            hasher.update(b"\0");
        }
        Commands {
            setup: runner.setup.clone(),
            build: runner.build.clone(),
            command: runner.command.clone(),
            hash: format!("blake3:{}", hasher.finalize().to_hex()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RedResult {
    Fail,
    /// The E2E did not build or load on red (it references code the feature adds), so no case could pass.
    DidNotBuild,
    /// Passed on red: the case does not bite. Allowed only with a justification in spec.md.
    NonDiscriminating,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GreenResult {
    Pass,
}

/// One failure mode across both runs.
#[derive(Debug, Serialize)]
pub struct Mode {
    pub red: RedResult,
    pub green: GreenResult,
    /// The names of the cases that proved it on green, sorted.
    pub cases: Vec<String>,
    /// The start of what the first failing case said on red: the assertion, not the stack.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// The Assay criteria tagged on the mode in spec.md.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub avp: Vec<String>,
    /// An explicit waiver remains visible in the proof, including its review attribution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avp_exemption: Option<super::avp_decision::Exemption>,
    /// The green verdict that decided a tagged mode, relative to the spec folder.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verdict: Option<String>,
    /// The verdict red's case saved, when it got that far: what the verifier said without the feature.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub red_verdict: Option<String>,
}

impl Receipt {
    /// Writes the receipt, unless the one on disk records the same proof: then it is kept as it is, so recording an
    /// unchanged spec again leaves git clean even after HEAD moved. Only the commits may differ for that; any other
    /// field (a result, a case, the runner's commands, the patch) is a new proof and is written. Returns whether it
    /// wrote.
    pub fn save(&self, spec: &SpecDir) -> Result<bool> {
        let path = spec.file(RECEIPT_FILE);
        let mut text = serde_json::to_string_pretty(self)?;
        text.push('\n');
        let current = std::fs::read_to_string(&path).ok();
        if current.as_deref().is_some_and(|current| same_proof(current, &text)) {
            return Ok(false);
        }
        std::fs::write(&path, text).with_context(|| format!("writing {}", path.display()))?;
        Ok(true)
    }
}

/// Whether two receipts record the same proof, apart from the commits red and green ran at.
fn same_proof(current: &str, new: &str) -> bool {
    let without_commits = |text: &str| {
        let mut value: serde_json::Value = serde_json::from_str(text).ok()?;
        for side in ["red", "green"] {
            value.get_mut(side)?.as_object_mut()?.remove("commit");
        }
        Some(value)
    };
    matches!((without_commits(current), without_commits(new)), (Some(a), Some(b)) if a == b)
}

#[cfg(test)]
mod tests {
    use super::same_proof;

    const RECEIPT: &str = r#"{ "spec": "0001-a", "red": { "commit": "aaa", "output": "fail" },
        "green": { "commit": "bbb" }, "failure_modes": { "FM-1": { "red": "fail", "green": "pass" } } }"#;

    #[test]
    fn only_the_commits_may_move_for_the_same_proof() {
        let moved = RECEIPT.replace("\"aaa\"", "\"ccc\"").replace("\"bbb\"", "\"ddd\"");
        assert!(same_proof(RECEIPT, &moved));
        let changed = RECEIPT.replace("\"red\": \"fail\"", "\"red\": \"did-not-build\"");
        assert!(!same_proof(RECEIPT, &changed));
        assert!(!same_proof("not json", RECEIPT));
    }
}
