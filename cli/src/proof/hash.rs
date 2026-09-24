//! Content hashes for footprints and inputs, and the file sets they cover.
//!
//! `proof status` must answer in milliseconds without running git, so everything here reads the filesystem
//! directly: blake3 over file bytes, in parallel, and `.gitignore`-aware walks for `touches` globs.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result};
use globset::{Glob, GlobSetBuilder};
use rayon::prelude::*;

use super::spec::{E2E_DIR, EVIDENCE_DIR, SPEC_FILE, SPECS_DIR, SpecDir};

/// Lockfiles at the project root that pin what the E2E runs against. A dependency bump changes behavior as
/// surely as a source edit, so they are inputs of every receipt.
pub const LOCKFILES: [&str; 5] = [
    "package-lock.json",
    "packages.lock.json",
    "pubspec.lock",
    "Cargo.lock",
    "Directory.Packages.props",
];

/// The recorded value for a footprint file that was deleted between red and green. Deleting a file is a change
/// the receipt covers, and it stays current as long as the file stays gone.
pub const ABSENT: &str = "absent";

/// Path (relative to the root, forward slashes) → `blake3:<hex>` or [`ABSENT`].
pub type Hashes = BTreeMap<String, String>;

pub fn hash_file(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    Some(format!("blake3:{}", blake3::hash(&bytes).to_hex()))
}

/// Hashes every path under `root` in parallel; a missing file hashes to [`ABSENT`].
pub fn hash_all<'a>(root: &Path, paths: impl IntoIterator<Item = &'a String>) -> Hashes {
    let paths: Vec<&String> = paths.into_iter().collect();
    paths
        .into_par_iter()
        .map(|rel| {
            (
                rel.clone(),
                hash_file(&root.join(rel)).unwrap_or_else(|| ABSENT.to_string()),
            )
        })
        .collect()
}

/// spec.md, every file under e2e/, and the root lockfiles that exist: what the receipt was recorded *from*.
pub fn input_paths(root: &Path, spec: &SpecDir) -> Result<BTreeSet<String>> {
    let mut paths = BTreeSet::from([format!("{}/{SPEC_FILE}", spec.rel())]);
    let e2e = spec.file(E2E_DIR);
    if e2e.is_dir() {
        for entry in walk(&e2e) {
            let entry = entry.with_context(|| format!("walking {}", e2e.display()))?;
            if entry.file_type().is_some_and(|kind| kind.is_file()) {
                paths.insert(relative(root, entry.path()));
            }
        }
    }
    paths.extend(
        LOCKFILES
            .iter()
            .filter(|name| root.join(name).is_file())
            .map(|name| name.to_string()),
    );
    Ok(paths)
}

/// Every committed file under the spec's evidence/, keyed relative to the spec folder (`evidence/avp-FM-2.json`).
/// evidence/raw/ (full reports and logs) is always left out, and the walk honors `.gitignore`, so regenerable
/// artifacts kept out of git are not part of the record and a fresh clone without them is not reported as tampered.
pub fn evidence(spec: &SpecDir) -> Result<Hashes> {
    let dir = spec.file(EVIDENCE_DIR);
    let raw = dir.join(super::evidence::RAW_DIR);
    let mut paths = BTreeSet::new();
    if dir.is_dir() {
        for entry in walk(&dir) {
            let entry = entry.with_context(|| format!("walking {}", dir.display()))?;
            if entry.file_type().is_some_and(|kind| kind.is_file()) && !entry.path().starts_with(&raw) {
                paths.insert(relative(&spec.path, entry.path()));
            }
        }
    }
    Ok(hash_all(&spec.path, &paths))
}

/// Files under `root` matching the spec's `touches` globs, honoring `.gitignore` so build output never lands in
/// a footprint. Each glob is walked from its literal prefix, so `src/Reservations/**` never scans the whole tree.
pub fn touched_paths(root: &Path, globs: &[String]) -> Result<BTreeSet<String>> {
    let mut found = BTreeSet::new();
    if globs.is_empty() {
        return Ok(found);
    }
    let mut builder = GlobSetBuilder::new();
    for glob in globs {
        builder.add(Glob::new(glob).with_context(|| format!("invalid touches glob '{glob}'"))?);
    }
    let set = builder.build()?;
    let starts: BTreeSet<&str> = globs.iter().map(|glob| literal_prefix(glob)).collect();
    for start in starts {
        let dir = root.join(start);
        if dir.is_file() {
            found.insert(start.to_string());
            continue;
        }
        if !dir.is_dir() {
            continue;
        }
        for entry in walk(&dir) {
            let entry = entry.with_context(|| format!("walking {}", dir.display()))?;
            if !entry.file_type().is_some_and(|kind| kind.is_file()) {
                continue;
            }
            let rel = relative(root, entry.path());
            if set.is_match(&rel) {
                found.insert(rel);
            }
        }
    }
    found.retain(|path| !is_spec_path(path));
    Ok(found)
}

/// A `.gitignore`-aware walk that still sees dotfiles (an e2e fixture may be one) but never enters `.git`.
fn walk(dir: &Path) -> ignore::Walk {
    ignore::WalkBuilder::new(dir)
        .hidden(false)
        .require_git(false)
        .filter_entry(|entry| entry.file_name() != ".git")
        .build()
}

/// The directory part of a glob before its first wildcard: `src/Res/**/*.cs` → `src/Res`.
fn literal_prefix(glob: &str) -> &str {
    let wild = glob.find(['*', '?', '[', '{']).unwrap_or(glob.len());
    match glob[..wild].rfind('/') {
        Some(slash) if wild < glob.len() => &glob[..slash],
        None if wild < glob.len() => "",
        _ => glob,
    }
}

/// Spec folders are never part of a footprint: a spec's own folder is its inputs, and another spec's receipt or
/// evidence changing must not make this one stale.
pub fn is_spec_path(path: &str) -> bool {
    path == SPECS_DIR || path.starts_with(&format!("{SPECS_DIR}/"))
}

pub fn relative(root: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    rel.components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Paths whose current hash differs from the recorded one, plus paths that are new since recording.
pub fn changed(recorded: &Hashes, current: &Hashes) -> Vec<String> {
    let mut changed: Vec<String> = current
        .iter()
        .filter(|(path, hash)| recorded.get(*path) != Some(*hash))
        .map(|(path, _)| path.clone())
        .collect();
    changed.extend(recorded.keys().filter(|path| !current.contains_key(*path)).cloned());
    changed.sort();
    changed.dedup();
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_prefixes() {
        assert_eq!(literal_prefix("src/Res/**"), "src/Res");
        assert_eq!(literal_prefix("src/Res/**/*.cs"), "src/Res");
        assert_eq!(literal_prefix("*.cs"), "");
        assert_eq!(literal_prefix("src/Pay.cs"), "src/Pay.cs");
    }

    #[test]
    fn walks_touches_and_skips_ignored_and_spec_files() {
        let root = tempfile::tempdir().unwrap();
        let write = |rel: &str| {
            let path = root.path().join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, rel).unwrap();
        };
        write("src/Res/Cancel.cs");
        write("src/Res/obj/gen.cs");
        write("src/Other.cs");
        write(".specs/0001-a/spec.md");
        std::fs::write(root.path().join(".gitignore"), "obj/\n").unwrap();

        let found = touched_paths(
            root.path(),
            &["src/Res/**".into(), "src/Other.cs".into(), "**/spec.md".into()],
        )
        .unwrap();
        assert_eq!(
            found.into_iter().collect::<Vec<_>>(),
            ["src/Other.cs", "src/Res/Cancel.cs"]
        );
    }

    #[test]
    fn detects_changed_new_and_missing_paths() {
        let recorded: Hashes = [
            ("a".into(), "1".into()),
            ("b".into(), "2".into()),
            ("c".into(), "3".into()),
        ]
        .into();
        let current: Hashes = [
            ("a".into(), "1".into()),
            ("b".into(), "9".into()),
            ("d".into(), "4".into()),
        ]
        .into();
        assert_eq!(changed(&recorded, &current), ["b", "c", "d"]);
    }
}
