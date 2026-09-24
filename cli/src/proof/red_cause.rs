//! Why red's cases never ran, and whether the spec's own e2e is the reason.
//!
//! A red run whose cases never ran counts as `did-not-build` only when the failure is attributable to the spec's
//! own cases: compiler errors located in its e2e files (.NET `error CS…`, TypeScript `error TS…`, Dart `Error:` and
//! analyzer errors, also in the copies a runner makes under `.skies_spec/`), or a report whose failed cases are
//! file-level failures of its e2e files (vitest's case for a test file whose import does not resolve, Flutter's
//! `loading …`). Anything else is not evidence about the feature: a restore that failed, a missing tool, a runner
//! command the shell could not run, an error in a file outside the spec, no output at all. `record` then stops
//! and writes no receipt, instead of calling every failure mode failing.

use std::path::Path;
use std::sync::LazyLock;

use anyhow::Result;
use regex::Regex;

use super::report::{Case, Outcome};
use super::spec::{E2E_DIR, SpecDir};

/// The folder a runner copies the spec's e2e into when its cases must sit inside a package (the Flutter runner).
const COPY_DIR: &str = ".skies_spec/";

/// The spec's e2e files, recognizable in the paths a compiler or a report prints.
pub struct SpecFiles {
    /// `.specs/<name>/e2e/`, which every path to one of its files contains, whatever the checkout or prefix.
    dir: String,
    /// Each file's path inside e2e/, for the copies under [`COPY_DIR`].
    files: Vec<String>,
}

impl SpecFiles {
    pub fn new(spec: &SpecDir) -> SpecFiles {
        let root = spec.file(E2E_DIR);
        let files = walkdir::WalkDir::new(&root)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
            .filter_map(|entry| relative(&root, entry.path()))
            .collect();
        SpecFiles {
            dir: format!("{}/{E2E_DIR}/", spec.rel()),
            files,
        }
    }

    /// Whether `path` (as a diagnostic or a report prints it) is one of the spec's e2e files or a runner's copy of one.
    pub fn owns(&self, path: &str) -> bool {
        let path = path.replace('\\', "/");
        if path.contains(&self.dir) {
            return true;
        }
        let Some(at) = path.find(COPY_DIR) else {
            return false;
        };
        let copied = &path[at + COPY_DIR.len()..];
        self.files.iter().any(|file| {
            copied
                .strip_prefix(file.as_str())
                .is_some_and(|rest| !rest.starts_with(|ch: char| ch.is_alphanumeric() || "_-./".contains(ch)))
        })
    }
}

fn relative(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let parts: Vec<String> = rel
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    Some(parts.join("/"))
}

/// The lines that tie red's failure to the spec's own e2e, or why it is not the spec's doing.
pub fn attribute(log: &str, cases: Option<&[Case]>, files: &SpecFiles) -> Result<Vec<String>, String> {
    let mut owned: Vec<String> = Vec::new();
    for line in log.lines() {
        let line = ANSI.replace_all(line, "");
        let line = line.trim();
        let Some(file) = diagnostic(line) else { continue };
        if !file.is_some_and(|file| files.owns(file)) {
            return Err(format!("an error outside the spec's e2e: {line}"));
        }
        if !owned.iter().any(|seen| seen == line) {
            owned.push(line.to_string());
        }
    }
    for case in cases
        .unwrap_or_default()
        .iter()
        .filter(|case| case.outcome == Outcome::Failed)
    {
        let first = case
            .message
            .as_deref()
            .and_then(|text| text.lines().next())
            .unwrap_or_default();
        let own = files.owns(&case.name) || case.file.as_deref().is_some_and(|file| files.owns(file));
        if !own {
            return Err(format!(
                "case \"{}\" failed and is not one of the spec's e2e files",
                case.name
            ));
        }
        let message = case.message.as_deref().unwrap_or_default();
        if let Some(outside) = outside_cause(message, files) {
            return Err(format!(
                "{} fails on {outside}, outside the spec's e2e: {first}",
                case.name
            ));
        }
        owned.push(
            format!("{}: {first}", case.name)
                .trim_end_matches([':', ' '])
                .to_string(),
        );
    }
    if owned.is_empty() {
        return Err(
            "nothing in its output points at the spec's e2e files (no compiler error located in them, no \
                    failed file of theirs in the report)"
                .into(),
        );
    }
    Ok(owned)
}

/// A compiler error line and the file it names (`None` when it names none), or `None` for any other line.
fn diagnostic(line: &str) -> Option<Option<&str>> {
    for pattern in [&*DOTNET, &*TYPESCRIPT, &*DART, &*ANALYZER] {
        if let Some(capture) = pattern.captures(line) {
            return Some(capture.name("file").map(|file| file.as_str().trim()));
        }
    }
    UNLOCATED.is_match(line).then_some(None)
}

/// What a file-level failure's message blames outside the spec's e2e, if anything: the file an unresolved import was
/// written in (vitest's `Failed to resolve import "x" from "<file>"`, Node's `… imported from <file>`), a compiler
/// error it quotes (a Dart load failure), or, when it names neither, an absolute path it could not load (a setup
/// file or a config the runner points at: `Cannot find module '/frontend-sdk/vitest.setup.ts'`).
fn outside_cause(message: &str, files: &SpecFiles) -> Option<String> {
    if let Some(capture) = IMPORTER.captures(message) {
        let importer = capture.get(1).or(capture.get(2)).map_or("", |file| file.as_str());
        return (!files.owns(importer)).then(|| format!("an import in {importer}"));
    }
    let mut located = false;
    for line in message.lines().map(str::trim) {
        if let Some(file) = diagnostic(line) {
            if !file.is_some_and(|file| files.owns(file)) {
                return Some(format!("a compiler error ({line})"));
            }
            located = true;
        }
    }
    if located {
        return None;
    }
    ABSOLUTE
        .captures_iter(message)
        .filter_map(|capture| capture.get(1))
        .map(|path| path.as_str())
        .find(|path| !files.owns(path))
        .map(String::from)
}

static ANSI: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\x1b\[[0-9;]*[A-Za-z]").expect("valid regex"));

/// MSBuild and the .NET SDK: `X.cs(3,5): error CS0246: …`, `/p/App.csproj : error NU1101: …`.
static DOTNET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?P<file>\S.*?)(?:\([0-9]+(?:,[0-9]+){1,3}\))?\s*:\s+(?:fatal )?error\s+[A-Z]+[0-9]+\s*:")
        .expect("valid regex")
});
/// `tsc --pretty`: `src/a.ts:3:5 - error TS2307: …`.
static TYPESCRIPT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?P<file>\S.*?):[0-9]+:[0-9]+ - error TS[0-9]+").expect("valid regex"));
/// The Dart front end (`flutter test`, `dart run`): `test/a_test.dart:3:8: Error: …`.
static DART: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?P<file>\S.*?\.dart):[0-9]+:[0-9]+: Error: ").expect("valid regex"));
/// `flutter analyze` (`error • msg • file:3:5 • code`) and `dart analyze` (`error - file:3:5 - msg`).
static ANALYZER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^error\s+(?:•.*?•|-)\s+(?P<file>\S+?):[0-9]+:[0-9]+\s+(?:•|-)").expect("valid regex")
});
/// An error with a code and no file: `error MSB1009: Project file does not exist.`
static UNLOCATED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:fatal )?error\s+[A-Z]+[0-9]+\s*:").expect("valid regex"));
static IMPORTER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)resolve import .*? from ["']([^"']+)["']|imported from ["']?([^"'\s]+)"#).expect("valid regex")
});
/// A quoted absolute path: `'/frontend-sdk/vitest.setup.ts'`, `"C:\app\setup.ts"`.
static ABSOLUTE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"["'`]((?:/|[A-Za-z]:[\\/])[^"'`\s]+)["'`]"#).expect("valid regex"));

#[cfg(test)]
#[path = "red_cause_tests.rs"]
mod tests;
