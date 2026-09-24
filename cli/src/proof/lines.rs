//! Executed-line fingerprints: what a coverage footprint records for each file the green run executed.
//!
//! A whole-file hash of a covered file goes stale on any edit, and every spec that boots a host executes every
//! slice's route mapping and every service registration, so editing one handler's body would mark every such spec
//! stale. A covered file is therefore recorded as the lines the run executed and a hash of their text: an edit
//! confined to lines the spec never ran leaves it current. Inserting or deleting lines above executed ones shifts
//! the text under the recorded numbers, which reads as stale; that is conservative and correct, since the receipt
//! can no longer say which code it ran. Files with no line data (the diff, `touches`, a summary-only LCOV record)
//! keep a whole-file hash, and a receipt whose covered files hold whole-file hashes reads exactly as before.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;
use std::str::FromStr;

use anyhow::{Result, bail};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use super::hash::ABSENT;

/// One footprint file as a receipt records it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Print {
    /// `blake3:<hex>` of the whole file, or [`ABSENT`] for a file deleted between red and green.
    Whole(String),
    /// The executed lines and a hash of their text.
    Lines(LinePrint),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinePrint {
    /// 1-based line numbers, compressed into ranges: `"12-18,40,55-60"`.
    pub lines: Ranges,
    /// blake3 over each recorded line's text (line ending dropped, inner whitespace kept) plus `\n`, in order.
    pub hash: String,
}

/// Path (relative to the root, forward slashes) → how the receipt pins it.
pub type Prints = BTreeMap<String, Print>;

/// Sorted, disjoint, non-adjacent inclusive ranges of 1-based line numbers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Ranges(Vec<(u32, u32)>);

impl Ranges {
    /// Folds a set of line numbers into runs; `None` for an empty set, which has nothing to pin. Line 0 does not
    /// exist and is dropped.
    pub fn from_set(lines: &BTreeSet<u32>) -> Option<Ranges> {
        let mut runs: Vec<(u32, u32)> = Vec::new();
        for &line in lines.iter().filter(|line| **line > 0) {
            match runs.last_mut() {
                Some((_, end)) if *end + 1 == line => *end = line,
                _ => runs.push((line, line)),
            }
        }
        (!runs.is_empty()).then_some(Ranges(runs))
    }

    /// Every line number, ascending.
    pub fn iter(&self) -> impl Iterator<Item = u32> + '_ {
        self.0.iter().flat_map(|(start, end)| *start..=*end)
    }

    /// The highest line number: the file must still have at least this many lines.
    pub fn last(&self) -> u32 {
        self.0.last().map_or(0, |(_, end)| *end)
    }
}

impl fmt::Display for Ranges {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, (start, end)) in self.0.iter().enumerate() {
            if index > 0 {
                f.write_str(",")?;
            }
            if start == end {
                write!(f, "{start}")?;
            } else {
                write!(f, "{start}-{end}")?;
            }
        }
        Ok(())
    }
}

impl FromStr for Ranges {
    type Err = anyhow::Error;

    /// Accepts only what [`Ranges::from_set`] writes, so a hand-edited or truncated range is an unreadable receipt,
    /// never a silently narrower one.
    fn from_str(text: &str) -> Result<Ranges> {
        let mut runs: Vec<(u32, u32)> = Vec::new();
        for part in text.split(',') {
            let (start, end) = match part.split_once('-') {
                Some((start, end)) => (number(start, text)?, number(end, text)?),
                None => {
                    let line = number(part, text)?;
                    (line, line)
                }
            };
            let after_previous = runs.last().is_none_or(|(_, previous)| start > previous + 1);
            if start == 0 || end < start || !after_previous {
                bail!("line ranges '{text}' are not ascending, disjoint runs of lines from 1");
            }
            runs.push((start, end));
        }
        Ok(Ranges(runs))
    }
}

fn number(part: &str, text: &str) -> Result<u32> {
    match part.parse::<u32>() {
        Ok(line) if !part.starts_with('+') => Ok(line),
        _ => bail!("line ranges '{text}': '{part}' is not a line number"),
    }
}

impl TryFrom<String> for Ranges {
    type Error = anyhow::Error;
    fn try_from(text: String) -> Result<Ranges> {
        text.parse()
    }
}

impl From<Ranges> for String {
    fn from(ranges: Ranges) -> String {
        ranges.to_string()
    }
}

/// A file as it is on disk now: its whole-file hash and where each line sits.
pub struct Content {
    hash: String,
    bytes: Vec<u8>,
    /// Byte span of each line, its `\n` or `\r\n` excluded.
    lines: Vec<(usize, usize)>,
}

impl Content {
    pub fn new(bytes: Vec<u8>) -> Content {
        let mut lines = Vec::new();
        let mut start = 0;
        for (index, byte) in bytes.iter().enumerate() {
            if *byte == b'\n' {
                lines.push((start, index));
                start = index + 1;
            }
        }
        if start < bytes.len() {
            lines.push((start, bytes.len()));
        }
        for (start, end) in &mut lines {
            if *end > *start && bytes[*end - 1] == b'\r' {
                *end -= 1;
            }
        }
        Content {
            hash: format!("blake3:{}", blake3::hash(&bytes).to_hex()),
            bytes,
            lines,
        }
    }

    /// The hash of the given lines' text, or `None` when the file has fewer lines than the highest of them.
    fn line_hash(&self, ranges: &Ranges) -> Option<String> {
        if ranges.last() as usize > self.lines.len() {
            return None;
        }
        let mut hasher = blake3::Hasher::new();
        for line in ranges.iter() {
            let (start, end) = self.lines[line as usize - 1];
            hasher.update(&self.bytes[start..end]);
            hasher.update(b"\n");
        }
        Some(format!("blake3:{}", hasher.finalize().to_hex()))
    }
}

/// The recorded print of a file: its executed lines when there are any and they all exist in today's text, else
/// the whole file (a coverage line past the end means the report and the file disagree, so pin everything).
pub fn print(content: Option<&Content>, executed: Option<&BTreeSet<u32>>) -> Print {
    let Some(content) = content else {
        return Print::Whole(ABSENT.to_string());
    };
    executed
        .and_then(Ranges::from_set)
        .and_then(|lines| {
            let hash = content.line_hash(&lines)?;
            Some(Print::Lines(LinePrint { lines, hash }))
        })
        .unwrap_or_else(|| Print::Whole(content.hash.clone()))
}

/// Whether today's file still matches what was recorded. A line print is stale when any recorded line's text
/// changed or the file no longer has that many lines; lines it does not name may change freely.
pub fn matches(recorded: &Print, content: Option<&Content>) -> bool {
    match recorded {
        Print::Whole(hash) => content.map_or(ABSENT, |content| content.hash.as_str()) == hash,
        Print::Lines(print) => {
            content.and_then(|content| content.line_hash(&print.lines)).as_ref() == Some(&print.hash)
        }
    }
}

/// Files read once each, in parallel: `status` checks every receipt against one snapshot, so a file shared by many
/// footprints (the host wiring, the entities) is read once. `None` is a file that does not exist.
#[derive(Default)]
pub struct Snapshot(BTreeMap<String, Option<Content>>);

impl Snapshot {
    pub fn read<'a>(root: &Path, paths: impl IntoIterator<Item = &'a String>) -> Snapshot {
        let paths: Vec<&String> = paths.into_iter().collect();
        Snapshot(
            paths
                .into_par_iter()
                .map(|rel| (rel.clone(), std::fs::read(root.join(rel)).ok().map(Content::new)))
                .collect(),
        )
    }

    /// `Some(None)` for a file read and found absent, `None` for a path this snapshot never read.
    pub fn get(&self, path: &str) -> Option<Option<&Content>> {
        self.0.get(path).map(Option::as_ref)
    }

    pub fn contains(&self, path: &str) -> bool {
        self.0.contains_key(path)
    }
}

/// Prints every footprint path from today's files: the ones in `executed` by their lines, the rest whole.
pub fn print_all(root: &Path, paths: &BTreeSet<String>, executed: &BTreeMap<String, BTreeSet<u32>>) -> Prints {
    let snapshot = Snapshot::read(root, paths);
    paths
        .iter()
        .map(|path| {
            let content = snapshot.get(path).flatten();
            (path.clone(), print(content, executed.get(path)))
        })
        .collect()
}

/// How many prints pin executed lines rather than the whole file, for the summary line.
pub fn by_lines(prints: &Prints) -> usize {
    prints.values().filter(|print| matches!(print, Print::Lines(_))).count()
}

#[cfg(test)]
#[path = "lines_tests.rs"]
mod tests;
