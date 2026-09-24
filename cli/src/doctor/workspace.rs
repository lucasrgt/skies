//! The workspace leg: every entry at the repository root must be declared in `Skies.toml` `[workspace] root`.
//!
//! Repositories accumulate junk at the root (logs, screenshots, stray notes, a folder someone needed once), and an
//! agent adds to it whenever it has nowhere better to put a file. An explicit allowlist turns "where does this go?"
//! into a decision the manifest records, and a new root entry into a one-line diff a reviewer sees.
//!
//! Matching: each `root` entry is a glob ([`globset`]) matched against one root entry's name. A trailing `/` makes it
//! match directories only; without one it matches files only, so `src/` and `README.md` say what they are and a file
//! named like a declared folder is still flagged. Matching is case-sensitive and `*` also matches dotfiles. `.git`
//! and `Skies.toml` are always allowed and never listed.
//!
//! What counts is what git would see: `.gitignore`d entries (build output, `node_modules/`, IDE state) never do, and
//! neither does a directory holding nothing git would see (an agent's ignored worktrees folder, an empty folder).
//! The leg reads one directory level plus, for each root directory, walks until its first visible file: no process
//! spawn, milliseconds on any repository.

use std::fs;
use std::path::{Path, PathBuf};

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};

use super::{Finding, Severity, Status};
use crate::manifest::FILE_NAME;

/// Root entries that are allowed without a declaration: the repository itself and the manifest that declares it.
const IMPLICIT: &[&str] = &[".git", FILE_NAME];

/// One entry at the repository root.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Entry {
    pub is_dir: bool,
    pub name: String,
}

impl Entry {
    /// The entry as it reads in a report and in `root`: a directory carries its trailing slash.
    pub fn display(&self) -> String {
        if self.is_dir {
            format!("{}/", self.name)
        } else {
            self.name.clone()
        }
    }

    /// The `root` pattern that declares exactly this entry: glob metacharacters are escaped as one-character classes.
    pub fn pattern(&self) -> String {
        let mut out = String::with_capacity(self.name.len() + 1);
        for c in self.name.chars() {
            match c {
                '*' | '?' | '[' | ']' | '{' | '}' | '\\' => {
                    out.push('[');
                    out.push(c);
                    out.push(']');
                }
                _ => out.push(c),
            }
        }
        if self.is_dir {
            out.push('/');
        }
        out
    }
}

/// The compiled `root` allowlist: one glob set for directories and one for files, each remembering its source entry.
pub struct Allowlist {
    dirs: GlobSet,
    dir_sources: Vec<String>,
    files: GlobSet,
    file_sources: Vec<String>,
}

impl Allowlist {
    pub fn compile(declared: &[String]) -> Result<Allowlist, String> {
        let (mut dirs, mut files) = (GlobSetBuilder::new(), GlobSetBuilder::new());
        let (mut dir_sources, mut file_sources) = (Vec::new(), Vec::new());
        for entry in declared {
            let (pattern, is_dir) = match entry.strip_suffix('/') {
                Some(dir) => (dir, true),
                None => (entry.as_str(), false),
            };
            if pattern.is_empty() || pattern.contains('/') {
                return Err(format!(
                    "`{entry}` is not a root entry: name one file or folder at the root (`docs/`, `*.slnx`), not a path"
                ));
            }
            let glob = GlobBuilder::new(pattern)
                .literal_separator(true)
                .build()
                .map_err(|error| format!("`{entry}` is not a valid glob: {error}"))?;
            if is_dir {
                dirs.add(glob);
                dir_sources.push(entry.clone());
            } else {
                files.add(glob);
                file_sources.push(entry.clone());
            }
        }
        let build = |builder: GlobSetBuilder| builder.build().map_err(|error| error.to_string());
        Ok(Allowlist {
            dirs: build(dirs)?,
            dir_sources,
            files: build(files)?,
            file_sources,
        })
    }

    pub fn allows(&self, entry: &Entry) -> bool {
        self.set(entry.is_dir).is_match(&entry.name)
    }

    fn set(&self, is_dir: bool) -> &GlobSet {
        if is_dir { &self.dirs } else { &self.files }
    }

    /// The declared entries no present entry matches.
    fn stale<'a>(&'a self, present: &[Entry]) -> Vec<&'a str> {
        let mut used = (
            vec![false; self.dir_sources.len()],
            vec![false; self.file_sources.len()],
        );
        for entry in present {
            let hits = if entry.is_dir { &mut used.0 } else { &mut used.1 };
            for index in self.set(entry.is_dir).matches(&entry.name) {
                hits[index] = true;
            }
        }
        let unused = |sources: &'a [String], hits: &[bool]| {
            sources
                .iter()
                .zip(hits.to_vec())
                .filter(|(_, hit)| !hit)
                .map(|(source, _)| source.as_str())
                .collect::<Vec<_>>()
        };
        let mut out = unused(&self.dir_sources, &used.0);
        out.extend(unused(&self.file_sources, &used.1));
        out
    }
}

/// The root entries git would see, sorted directories first, then by name.
pub fn visible_entries(root: &Path) -> Vec<Entry> {
    let mut out: Vec<Entry> = walker(root)
        .max_depth(Some(1))
        .build()
        .flatten()
        .filter(|item| item.depth() == 1)
        .filter_map(|item| entry(item.path()))
        .filter(|entry| !entry.is_dir || holds_a_visible_file(&root.join(&entry.name)))
        .collect();
    out.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.cmp(&b.name)));
    out
}

/// Every root entry on disk, ignored or not: what a declared entry may legitimately name.
fn all_entries(root: &Path) -> Vec<Entry> {
    fs::read_dir(root)
        .map(|dir| dir.flatten().filter_map(|item| entry(&item.path())).collect())
        .unwrap_or_default()
}

fn entry(path: &Path) -> Option<Entry> {
    let name = path.file_name()?.to_string_lossy().into_owned();
    (!IMPLICIT.contains(&name.as_str())).then(|| Entry {
        is_dir: path.is_dir(),
        name,
    })
}

fn walker(dir: &Path) -> ignore::WalkBuilder {
    let mut builder = ignore::WalkBuilder::new(dir);
    builder
        .hidden(false)
        .require_git(false)
        .filter_entry(|entry| entry.file_name() != ".git");
    builder
}

fn holds_a_visible_file(dir: &Path) -> bool {
    dir.is_symlink()
        || walker(dir)
            .build()
            .flatten()
            .any(|item| item.file_type().is_some_and(|kind| !kind.is_dir()))
}

/// Checks the root against `declared`; `None` means the manifest has no `root` yet.
pub fn check(root: &Path, declared: Option<&[String]>) -> (Status, Vec<Finding>) {
    let manifest = root.join(FILE_NAME);
    let text = fs::read_to_string(&manifest).unwrap_or_default();
    let visible = visible_entries(root);
    let Some(declared) = declared else {
        let patterns: Vec<String> = visible.iter().map(Entry::pattern).collect();
        let message = format!(
            "{FILE_NAME} [workspace] declares no `root`: list what may sit at the repository root (today's entries, \
             ready to paste under [workspace]; then delete the junk and trim the list):\n{}",
            indent(&render_root(&patterns))
        );
        let line = line_of(&text, |line| line.trim_start().starts_with("[workspace]"));
        return (
            Status::Ran,
            vec![Finding::new("SKYWS001", Severity::Error, manifest, line, message)],
        );
    };
    let allowlist = match Allowlist::compile(declared) {
        Ok(allowlist) => allowlist,
        Err(reason) => {
            return (
                Status::Failed(format!("{FILE_NAME} [workspace] root: {reason}")),
                Vec::new(),
            );
        }
    };
    let mut findings: Vec<Finding> = visible
        .iter()
        .filter(|entry| !allowlist.allows(entry))
        .map(|entry| undeclared(root, entry, &allowlist))
        .collect();
    for stale in allowlist.stale(&all_entries(root)) {
        let line = line_of(&text, |line| {
            line.contains(&format!("\"{stale}\"")) || line.contains(&format!("'{stale}'"))
        });
        findings.push(Finding::new(
            "SKYWS002",
            Severity::Warning,
            manifest.clone(),
            line,
            format!("`{stale}` in {FILE_NAME} [workspace] root matches nothing at the repository root; remove it"),
        ));
    }
    (Status::Ran, findings)
}

fn undeclared(root: &Path, entry: &Entry, allowlist: &Allowlist) -> Finding {
    let mut message = format!(
        "{} is not declared in {FILE_NAME} [workspace] root; move it under a declared folder, delete it, or declare it",
        entry.display()
    );
    if allowlist.set(!entry.is_dir).is_match(&entry.name) {
        message.push_str(if entry.is_dir {
            " (it is declared as a file; a folder takes a trailing slash)"
        } else {
            " (it is declared as a folder; a file takes no trailing slash)"
        });
    }
    let file: PathBuf = root.join(&entry.name);
    Finding::new("SKYWS001", Severity::Error, file, None, message)
}

/// `root = [...]` as TOML, one entry per line, each quoted by the TOML writer (so a name like `".skies` survives).
pub fn render_root(patterns: &[String]) -> String {
    let mut out = String::from("root = [\n");
    for pattern in patterns {
        out.push_str("  ");
        out.push_str(&toml::Value::String(pattern.clone()).to_string());
        out.push_str(",\n");
    }
    out.push_str("]\n");
    out
}

fn indent(text: &str) -> String {
    text.lines()
        .map(|line| format!("    {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn line_of(text: &str, predicate: impl Fn(&str) -> bool) -> Option<usize> {
    text.lines().position(predicate).map(|index| index + 1)
}

#[cfg(test)]
#[path = "workspace_tests.rs"]
mod tests;
