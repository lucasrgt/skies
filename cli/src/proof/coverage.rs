//! Coverage reports: which project files a green run actually executed.
//!
//! A footprint built from the red..green diff misses every shared file the feature relies on without editing it
//! (middleware, an entity, the platform wiring), so a change there never makes the receipt stale. When a runner
//! writes coverage, the engine reads it back and the footprint becomes the files the run executed, plus the diff.
//!
//! Two formats cover the three platforms, told apart by content: Cobertura XML (coverlet for .NET) and LCOV
//! (vitest's `lcov` reporter, `flutter test --coverage`). Each file with at least one executed line is kept with the
//! numbers of those lines, which the receipt pins by their text (see `lines`); the report itself is never stored.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// The project files a run executed, relative to the project root with forward slashes, each with the line numbers
/// it executed. An empty set means executed but with no line data (an LCOV record with only an `LH:` summary), which
/// the receipt pins as the whole file.
pub type Covered = BTreeMap<String, BTreeSet<u32>>;

/// Adds one report's lines for a file. A file without line data in any report stays without: its lines are unknown.
fn merge(covered: &mut Covered, file: String, lines: &BTreeSet<u32>) {
    match covered.entry(file) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(lines.clone());
        }
        std::collections::btree_map::Entry::Occupied(mut entry) => {
            if entry.get().is_empty() || lines.is_empty() {
                entry.get_mut().clear();
            } else {
                entry.get_mut().extend(lines);
            }
        }
    }
}

/// Where a runner's coverage lands, and whether the runner was set up to write any.
pub struct Location {
    /// A file, or a directory searched for coverage files (coverlet nests its output under a per-run GUID).
    pub path: PathBuf,
    /// The runner declares `coverage` or uses `{coverage}`. A missing file is only worth a warning when it is.
    pub configured: bool,
}

/// What a run's coverage says about the footprint.
pub enum Outcome {
    Covered(Covered),
    /// No coverage to read; the reason is printed where the footprint falls back to the diff.
    Missing(String),
}

/// Reads every coverage file at `location` and maps what it executed onto the project. `cwd` is where the runner
/// ran (relative paths in a report resolve against it first); `roots` are the checkouts' project roots, so an
/// absolute path under any of them maps back to a project path.
pub fn collect(location: &Location, runner: &str, cwd: &Path, roots: &[&Path]) -> Result<Outcome> {
    let shown = location.path.display();
    if !location.configured && !location.path.exists() {
        return Ok(Outcome::Missing(format!(
            "runner '{runner}' writes no coverage; add `coverage` or `{{coverage}}` to it for a footprint of the files the run executes"
        )));
    }
    let files = coverage_files(&location.path)?;
    if files.is_empty() {
        return Ok(Outcome::Missing(format!(
            "runner '{runner}' wrote no coverage at {shown}"
        )));
    }
    let roots = Roots::new(roots);
    let mut covered = Covered::new();
    let mut read_any = false;
    for file in files {
        let text = std::fs::read_to_string(&file).with_context(|| format!("reading {}", file.display()))?;
        let Some(parsed) = parse(&text).with_context(|| format!("parsing the coverage {}", file.display()))? else {
            continue;
        };
        read_any = true;
        // A monorepo tool (vitest run from the repository root) names files relative to a directory above the
        // project, so the project's ancestors are candidates too; only a name that exists on disk is accepted.
        let mut dirs: Vec<&Path> = cwd.ancestors().collect();
        dirs.extend(file.ancestors().skip(1));
        for (raw, lines) in &parsed.files {
            let path = resolve(raw, &parsed.sources, &dirs);
            if let Some(rel) = roots.relative(&path).filter(|rel| is_source(rel)) {
                merge(&mut covered, rel, lines);
            }
        }
    }
    if !read_any {
        return Ok(Outcome::Missing(format!(
            "runner '{runner}' wrote nothing at {shown} that reads as Cobertura or LCOV"
        )));
    }
    Ok(Outcome::Covered(covered))
}

/// `path` itself, or every file under it when it is a directory, in a stable order.
fn coverage_files(path: &Path) -> Result<Vec<PathBuf>> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }
    if !path.is_dir() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(path).sort_by_file_name() {
        let entry = entry.with_context(|| format!("walking {}", path.display()))?;
        let name = entry.file_name().to_string_lossy();
        let candidate = name.ends_with(".xml") || name.ends_with(".info") || name.ends_with(".lcov");
        if entry.file_type().is_file() && candidate {
            files.push(entry.into_path());
        }
    }
    Ok(files)
}

/// One report's executed files, as the report spells them, each with its executed line numbers, and the base
/// directories it declares for relative names (Cobertura's `<source>`; LCOV has none).
#[derive(Debug, Default, PartialEq)]
pub struct Parsed {
    pub sources: Vec<String>,
    pub files: Covered,
}

/// Cobertura or LCOV, by content; `None` for anything else (a TRX or JUnit file that shares the folder).
pub fn parse(text: &str) -> Result<Option<Parsed>> {
    let head = text.trim_start_matches('\u{feff}').trim_start();
    if head.starts_with('<') {
        return if head.contains("<coverage") {
            cobertura(text).map(Some)
        } else {
            Ok(None)
        };
    }
    if text.lines().any(|line| line.starts_with("SF:")) {
        return Ok(Some(lcov(text)));
    }
    Ok(None)
}

/// A file counts when any `<line>` of any of its classes has `hits` above zero, and its executed lines are the union
/// over those classes; a file split across partial classes or nested types appears once per class.
fn cobertura(text: &str) -> Result<Parsed> {
    let options = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    let document = roxmltree::Document::parse_with_options(text, options)?;
    let mut parsed = Parsed::default();
    for node in document.descendants() {
        match node.tag_name().name() {
            "source" => {
                if let Some(source) = node.text().map(str::trim).filter(|source| !source.is_empty()) {
                    parsed.sources.push(source.to_string());
                }
            }
            "class" => {
                let Some(filename) = node.attribute("filename") else {
                    continue;
                };
                let mut executed = false;
                let mut lines = BTreeSet::new();
                for line in node.descendants().filter(|line| line.has_tag_name("line")) {
                    if line.attribute("hits").is_some_and(positive) {
                        executed = true;
                        lines.extend(line.attribute("number").and_then(line_number));
                    }
                }
                if executed {
                    merge(&mut parsed.files, filename.to_string(), &lines);
                }
            }
            _ => {}
        }
    }
    Ok(parsed)
}

/// A record (`SF:` … `end_of_record`) counts when a `DA:<line>,<hits>` has hits above zero, those lines being the
/// executed ones, or, for a tool that writes only the summary, when `LH:` is above zero (no line data).
fn lcov(text: &str) -> Parsed {
    let mut parsed = Parsed::default();
    let mut current: Option<&str> = None;
    let mut executed = false;
    let mut lines = BTreeSet::new();
    for line in text.lines().map(str::trim) {
        if let Some(file) = line.strip_prefix("SF:") {
            current = Some(file.trim());
            executed = false;
            lines.clear();
        } else if let Some(data) = line.strip_prefix("DA:") {
            let mut fields = data.split(',');
            let number = fields.next().and_then(line_number);
            if fields.next().is_some_and(positive) {
                executed = true;
                lines.extend(number);
            }
        } else if let Some(hit) = line.strip_prefix("LH:") {
            executed |= positive(hit);
        } else if line == "end_of_record" {
            if let Some(file) = current.take().filter(|_| executed) {
                merge(&mut parsed.files, file.to_string(), &lines);
            }
            executed = false;
            lines.clear();
        }
    }
    if let Some(file) = current.filter(|_| executed) {
        merge(&mut parsed.files, file.to_string(), &lines);
    }
    parsed
}

fn positive(count: &str) -> bool {
    count.trim().parse::<f64>().is_ok_and(|count| count > 0.0)
}

/// A 1-based line number; anything else (0, a range, garbage) is no line data rather than an error.
fn line_number(text: &str) -> Option<u32> {
    text.trim().parse::<u32>().ok().filter(|line| *line > 0)
}

/// A report path made absolute: as written when absolute, else under the first base where the file exists (the
/// report's declared sources, then the runner's directory and its parents, then the report's own folder and its
/// parents, which is where Flutter's and vitest's relative `SF:` names start). A name found nowhere keeps its first candidate.
fn resolve(raw: &str, sources: &[String], dirs: &[&Path]) -> PathBuf {
    let raw = raw.replace('\\', "/");
    let path = Path::new(&raw);
    if path.is_absolute() || is_drive_path(&raw) {
        return path.to_path_buf();
    }
    let candidates: Vec<PathBuf> = sources
        .iter()
        .map(|source| Path::new(source).join(path))
        .chain(dirs.iter().map(|dir| dir.join(path)))
        .collect();
    candidates
        .iter()
        .find(|candidate| candidate.is_file())
        .or(candidates.first())
        .cloned()
        .unwrap_or_else(|| path.to_path_buf())
}

/// `C:/src/App.cs` from a Windows report read elsewhere: absolute, though not to this platform's `Path`.
fn is_drive_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() > 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'/'
}

/// The project roots a covered path may sit under, each spelled as given and canonically, with forward slashes.
struct Roots(Vec<String>);

impl Roots {
    fn new(roots: &[&Path]) -> Roots {
        let mut spelled = Vec::new();
        for root in roots {
            let canonical = std::fs::canonicalize(root).ok();
            for variant in std::iter::once(root.to_path_buf()).chain(canonical) {
                let text = format!(
                    "{}/",
                    variant.to_string_lossy().replace('\\', "/").trim_end_matches('/')
                );
                if !spelled.contains(&text) {
                    spelled.push(text);
                }
            }
        }
        // Longest first, so a worktree nested under the project root maps to its own root.
        spelled.sort_by_key(|root| std::cmp::Reverse(root.len()));
        Roots(spelled)
    }

    /// The project path of `path`, or `None` when it lies outside every root (a framework package, the SDK).
    fn relative(&self, path: &Path) -> Option<String> {
        let text = normalize(&path.to_string_lossy().replace('\\', "/"));
        self.0.iter().find_map(|root| {
            let rest = if cfg!(windows) {
                (text.len() > root.len() && text[..root.len()].eq_ignore_ascii_case(root)).then(|| &text[root.len()..])
            } else {
                text.strip_prefix(root.as_str())
            };
            rest.filter(|rest| !rest.is_empty()).map(String::from)
        })
    }
}

/// Folds `.` and `..` segments lexically, so `/app/src/../src/A.cs` compares equal to `/app/src/A.cs`.
fn normalize(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "." => {}
            ".." if parts.last().is_some_and(|last| !last.is_empty() && *last != "..") => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    parts.join("/")
}

/// Build output and generated code are never footprint: they change with every build, or with the source that
/// generates them, which is already there. Spec folders never are either (see `hash::is_spec_path`).
fn is_source(rel: &str) -> bool {
    const DIRS: [&str; 5] = ["obj", "bin", "node_modules", ".dart_tool", "client.gen"];
    const SUFFIXES: [&str; 5] = [".g.cs", ".g.dart", ".freezed.dart", ".designer.cs", ".generated.cs"];
    let lower = rel.to_ascii_lowercase();
    let mut dirs = rel.split('/').rev().skip(1);
    !super::hash::is_spec_path(rel)
        && !dirs.any(|dir| DIRS.contains(&dir))
        && !SUFFIXES.iter().any(|suffix| lower.ends_with(suffix))
}

#[cfg(test)]
#[path = "coverage_tests.rs"]
mod tests;
