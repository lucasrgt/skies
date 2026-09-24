//! The footprint: which files a receipt depends on, so a change to any of them makes it stale.
//!
//! With coverage it is every project file the green run executed, plus what changed between red and green (a
//! deleted file, a config the run reads without executing), plus `touches`. Without coverage it is the diff plus
//! `touches`, as before, and the receipt says so. Either way spec folders never belong to it, and a module ctx.md
//! joins only through `touches`: prose is kept fresh by citation (SKY0005), not by hash.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::coverage::Outcome;
use super::hash;
use super::spec::SpecDoc;
use super::{ctx, green::ProvenGreen};

/// Where a receipt's footprint came from.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// The files the green run executed, plus the diff and `touches`.
    Coverage,
    /// The files changed between red and green, plus `touches`: the fallback, and every receipt written before
    /// coverage existed.
    #[default]
    Diff,
}

pub struct Footprint {
    pub paths: BTreeSet<String>,
    pub source: Source,
    /// The changed part, recorded with a coverage footprint so `verify` can refresh the covered part alone.
    pub changed: Vec<String>,
    /// How many files the coverage contributed, for the summary line.
    pub covered: usize,
}

/// Builds the footprint from the diff (`changed`), `touches`, and the green run's coverage when it has any.
pub fn build(root: &Path, doc: &SpecDoc, changed: &[String], proven: &ProvenGreen) -> Result<Footprint> {
    let artifact = proven.coverage_artifact.as_deref();
    let keep = |path: &String| {
        !hash::is_spec_path(path) && !artifact.is_some_and(|dir| path == dir || path.starts_with(&format!("{dir}/")))
    };
    let changed: Vec<String> = changed
        .iter()
        .filter(|path| !ctx::is_ctx(path) && keep(path))
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut paths: BTreeSet<String> = changed.iter().cloned().collect();
    paths.extend(hash::touched_paths(root, &doc.touches)?);
    let (source, covered) = match &proven.coverage {
        Outcome::Covered(covered) => {
            paths.extend(covered.iter().filter(|path| keep(path)).cloned());
            (Source::Coverage, covered.len())
        }
        Outcome::Missing(_) => (Source::Diff, 0),
    };
    paths.retain(|path| keep(path));
    let changed = if source == Source::Coverage {
        changed
    } else {
        Vec::new()
    };
    Ok(Footprint {
        paths,
        source,
        changed,
        covered,
    })
}

/// One line on how the footprint was found, so a fallback is never silent.
pub fn describe(footprint: &Footprint, proven: &ProvenGreen) -> String {
    match &proven.coverage {
        Outcome::Covered(_) => format!(
            "footprint from coverage: {} executed + {} changed since red + touches = {}",
            footprint.covered,
            footprint.changed.len(),
            files(footprint.paths.len())
        ),
        Outcome::Missing(why) => format!(
            "footprint from the diff since red + touches ({}): {why}",
            files(footprint.paths.len())
        ),
    }
}

fn files(count: usize) -> String {
    format!("{count} file{}", if count == 1 { "" } else { "s" })
}

/// The covered set a coverage-less receipt upgrades from: its recorded files minus what `touches` adds today, which
/// are the files changed since red.
pub fn recorded_changed(recorded: &BTreeSet<String>, touched: &BTreeSet<String>) -> Vec<String> {
    recorded.difference(touched).cloned().collect()
}
