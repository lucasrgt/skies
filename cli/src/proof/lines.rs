//! Executed-line fingerprints: what a coverage footprint records for each file the green run executed.
//!
//! A whole-file hash of a covered file goes stale on any edit, and every spec that boots a host executes every
//! slice's route mapping and every service registration, so editing one handler's body would mark every such spec
//! stale. A covered file is therefore recorded as the lines the run executed, one hash per contiguous run of them:
//! an edit confined to lines the spec never ran leaves it current. Lines inserted or deleted outside the executed
//! runs only move them, so each run that is not at its recorded place is looked for further on, in order: the
//! receipt stays current as long as every run's text is still there, in the same order, without overlapping. It is
//! stale when some run's text changed or can no longer be found after the one before it. Files with no line data
//! (the diff, `touches`, a summary-only LCOV record) keep a whole-file hash. Receipts written before per-run hashes
//! (one `hash` over every executed line) still read, by position, as they did then.

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
#[serde(try_from = "RawLinePrint", into = "RawLinePrint")]
pub struct LinePrint {
    /// 1-based line numbers, compressed into ranges: `"12-18,40,55-60"`.
    pub lines: Ranges,
    pub hash: LineHash,
}

/// How a line print's text is hashed. Each hash is blake3 over the lines' text (line ending dropped, inner
/// whitespace kept), each followed by `\n`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineHash {
    /// One hash per range, in order: the first 64 bits of blake3, as 16 hex digits. Matched shift-tolerantly.
    PerRange(Vec<String>),
    /// One `blake3:<hex>` over every recorded line, the format before per-range hashes. Matched by position.
    Joined(String),
}

/// The JSON shape: `{"lines": "12-18,40", "ranges": "9f…,03…"}`, or `{"lines": …, "hash": "blake3:…"}` as written
/// before per-range hashes.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLinePrint {
    lines: Ranges,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ranges: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    hash: Option<String>,
}

impl TryFrom<RawLinePrint> for LinePrint {
    type Error = anyhow::Error;
    fn try_from(raw: RawLinePrint) -> Result<LinePrint> {
        let hash = match (raw.ranges, raw.hash) {
            (Some(ranges), None) => {
                let hashes: Vec<String> = ranges.split(',').map(String::from).collect();
                let well_formed = hashes.iter().all(|hash| {
                    hash.len() == 16 && hash.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                });
                if !well_formed || hashes.len() != raw.lines.0.len() {
                    bail!(
                        "range hashes '{ranges}' are not one 16-digit hex hash per range of '{}'",
                        raw.lines
                    );
                }
                LineHash::PerRange(hashes)
            }
            (None, Some(hash)) => LineHash::Joined(hash),
            _ => bail!("a line print has either `ranges` or `hash`"),
        };
        Ok(LinePrint { lines: raw.lines, hash })
    }
}

impl From<LinePrint> for RawLinePrint {
    fn from(print: LinePrint) -> RawLinePrint {
        let (ranges, hash) = match print.hash {
            LineHash::PerRange(hashes) => (Some(hashes.join(",")), None),
            LineHash::Joined(hash) => (None, Some(hash)),
        };
        RawLinePrint {
            lines: print.lines,
            ranges,
            hash,
        }
    }
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

    /// Each run as its 0-based first line and its length.
    fn runs(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.0
            .iter()
            .map(|(start, end)| (*start as usize - 1, (end - start) as usize + 1))
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

    /// The joined hash of the given lines' text (the format before per-range hashes), or `None` when the file has
    /// fewer lines than the highest of them.
    fn line_hash(&self, ranges: &Ranges) -> Option<String> {
        if ranges.last() as usize > self.lines.len() {
            return None;
        }
        let mut hasher = blake3::Hasher::new();
        for line in ranges.iter() {
            self.feed(&mut hasher, line as usize - 1);
        }
        Some(format!("blake3:{}", hasher.finalize().to_hex()))
    }

    /// The per-range hash of `count` lines from the 0-based line `first`, which must exist.
    fn run_hash(&self, first: usize, count: usize) -> String {
        let mut hasher = blake3::Hasher::new();
        for line in first..first + count {
            self.feed(&mut hasher, line);
        }
        hasher.finalize().to_hex()[..16].to_string()
    }

    fn feed(&self, hasher: &mut blake3::Hasher, line: usize) {
        let (start, end) = self.lines[line];
        hasher.update(&self.bytes[start..end]);
        hasher.update(b"\n");
    }

    /// Whether every run's text is found in order, without overlap: at its recorded place moved by however far the
    /// run before it moved, or else at the first place after the run before it. Unchanged files check each run once.
    fn runs_in_order(&self, ranges: &Ranges, hashes: &[String]) -> bool {
        let total = self.lines.len();
        let mut next = 0;
        let mut shift: isize = 0;
        for ((first, count), hash) in ranges.runs().zip(hashes) {
            let expected = first as isize + shift;
            let fits = |at: usize| at + count <= total && self.run_hash(at, count) == *hash;
            let found = usize::try_from(expected)
                .ok()
                .filter(|at| *at >= next && fits(*at))
                .or_else(|| (next..total.saturating_sub(count) + 1).find(|at| fits(*at)));
            let Some(at) = found else {
                return false;
            };
            shift = at as isize - first as isize;
            next = at + count;
        }
        true
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
        .filter(|lines| lines.last() as usize <= content.lines.len())
        .map(|lines| {
            let hashes = lines
                .runs()
                .map(|(first, count)| content.run_hash(first, count))
                .collect();
            Print::Lines(LinePrint {
                lines,
                hash: LineHash::PerRange(hashes),
            })
        })
        .unwrap_or_else(|| Print::Whole(content.hash.clone()))
}

/// Whether today's file still matches what was recorded. A line print is stale when some executed run's text
/// changed or can no longer be found in order (a print from before per-range hashes: when any recorded line's text
/// changed or moved); lines it does not name may change freely.
pub fn matches(recorded: &Print, content: Option<&Content>) -> bool {
    match recorded {
        Print::Whole(hash) => content.map_or(ABSENT, |content| content.hash.as_str()) == hash,
        Print::Lines(print) => content.is_some_and(|content| match &print.hash {
            LineHash::PerRange(hashes) => content.runs_in_order(&print.lines, hashes),
            LineHash::Joined(hash) => content.line_hash(&print.lines).as_ref() == Some(hash),
        }),
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
