//! `receipt.json`: the record that a spec's failure modes failed before the change and passed after it.
//!
//! It proves red→green once; CI keeps green passing from then on, so nothing here pins files or detects drift. Keys
//! are written in a fixed order (struct order, then sorted maps) and nothing in it varies between two runs of the
//! same code (no timestamps, durations, or report hashes), so re-recording an unchanged spec rewrites nothing.

use std::collections::BTreeMap;

use anyhow::{Context, Result};
use serde::Serialize;

use super::report::FmId;
use super::spec::{RECEIPT_FILE, SpecDir};

#[derive(Debug, Serialize)]
pub struct Receipt {
    pub spec: String,
    pub runner: String,
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
    /// The green verdict that decided a tagged mode, relative to the spec folder.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verdict: Option<String>,
    /// The verdict red's case saved, when it got that far: what the verifier said without the feature.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub red_verdict: Option<String>,
}

impl Receipt {
    /// Writes the receipt, leaving the file alone when its bytes would not change.
    pub fn save(&self, spec: &SpecDir) -> Result<()> {
        let path = spec.file(RECEIPT_FILE);
        let mut text = serde_json::to_string_pretty(self)?;
        text.push('\n');
        if std::fs::read_to_string(&path).is_ok_and(|current| current == text) {
            return Ok(());
        }
        std::fs::write(&path, text).with_context(|| format!("writing {}", path.display()))
    }
}
