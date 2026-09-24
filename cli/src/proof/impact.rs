//! `skies proof impact`: which specs a change reaches, read straight from the receipts.
//!
//! Every receipt already lists the files its feature covers (the footprint), and every spec.md may widen that with
//! `touches` globs. Inverting those gives path → specs, so the receipts are the index: there is no index file to
//! build, commit, or let drift. Nothing is hashed to answer "which specs"; only the specs that turn out impacted
//! are rehashed, to say whether their receipt is current. The ctx.md of every module the paths reach is named too,
//! since its invariants are what a new failure mode must not contradict.

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};

use super::ctx;
use super::git::Repo;
use super::receipt::Receipt;
use super::spec::{self, SpecDir, SpecDoc};
use crate::manifest::Project;

/// One spec as the index sees it: what it covers, and the failure modes a reader must keep true.
pub struct Indexed {
    pub spec: SpecDir,
    pub doc: SpecDoc,
    footprint: BTreeSet<String>,
    touches: GlobSet,
}

impl Indexed {
    /// Whether a change to `path` (a file or a directory, relative to the project root) can reach this spec: it is
    /// in the recorded footprint, matches a `touches` glob, or sits in the spec's own folder.
    pub fn covers(&self, path: &str) -> bool {
        let dir = format!("{}/", path.trim_end_matches('/'));
        let own = self.spec.rel();
        self.footprint.contains(path)
            || self.touches.is_match(path)
            || path == own
            || path.starts_with(&format!("{own}/"))
            || self.footprint.iter().any(|file| file.starts_with(&dir))
    }
}

/// Every spec with its receipt footprint and `touches`. A spec without a receipt is still indexed by its globs.
pub fn index(root: &Path) -> Result<Vec<Indexed>> {
    let mut indexed = Vec::new();
    for spec in spec::discover(root)? {
        let doc = SpecDoc::load(&spec)?;
        let footprint = Receipt::load(&spec)?
            .map(|receipt| receipt.footprint.into_keys().collect())
            .unwrap_or_default();
        let mut builder = GlobSetBuilder::new();
        for glob in &doc.touches {
            builder.add(Glob::new(glob).with_context(|| format!("{}: invalid touches glob '{glob}'", spec.rel()))?);
        }
        indexed.push(Indexed {
            touches: builder.build()?,
            footprint,
            doc,
            spec,
        });
    }
    Ok(indexed)
}

/// The specs other than `name` that a change to any of `paths` reaches: what `record --with-impacted` re-proves.
pub fn impacted_by<'a>(index: &'a [Indexed], name: &str, paths: &BTreeSet<String>) -> Vec<&'a Indexed> {
    index
        .iter()
        .filter(|entry| entry.spec.name != name && paths.iter().any(|path| entry.covers(path)))
        .collect()
}

pub fn impact(paths: &[PathBuf], diff: Option<&str>) -> Result<u8> {
    let project = Project::from_cwd()?;
    let root = project.root.as_path();
    let changed: Vec<String> = if paths.is_empty() || diff.is_some() {
        let repo = Repo::open(root)?;
        let rev = match diff.filter(|rev| !rev.is_empty()) {
            Some(rev) => repo.resolve(rev)?,
            None => repo.fork_point()?,
        };
        let mut changed = repo.changed_since(&rev)?;
        let cwd = std::env::current_dir()?;
        changed.extend(paths.iter().map(|path| project_path(root, &cwd, path)));
        changed
    } else {
        let cwd = std::env::current_dir()?;
        paths.iter().map(|path| project_path(root, &cwd, path)).collect()
    };
    let changed: BTreeSet<String> = changed.into_iter().collect();
    if changed.is_empty() {
        println!("no changed files");
        return Ok(0);
    }

    let index = index(root)?;
    let mut uncovered: Vec<&String> = Vec::new();
    let mut hits: Vec<(&Indexed, Vec<&String>)> = index.iter().map(|entry| (entry, Vec::new())).collect();
    for path in &changed {
        let mut covered = false;
        for (entry, via) in hits.iter_mut() {
            if entry.covers(path) {
                via.push(path);
                covered = true;
            }
        }
        if !covered {
            uncovered.push(path);
        }
    }
    hits.retain(|(_, via)| !via.is_empty());

    for (entry, via) in &hits {
        println!("{}  {}", entry.spec.name, super::freshness_line(root, &entry.spec).0);
        println!("  via {}", abbreviate(via));
        for id in &entry.doc.failure_modes {
            let mode = &entry.doc.modes[id];
            let tag = if mode.avp.is_empty() {
                String::new()
            } else {
                format!(" [avp: {}]", mode.avp.join(", "))
            };
            println!("  - {id} {}{tag}", mode.text);
        }
    }
    if !uncovered.is_empty() {
        let noun = if uncovered.len() == 1 { "path" } else { "paths" };
        println!("{} {noun} in no spec: {}", uncovered.len(), abbreviate(&uncovered));
    }
    // The invariants of every module the change reaches, and the specs they cite: read them before writing the
    // failure modes, so a new mode does not contradict one the module already promises.
    for file in ctx::touched(root, &changed) {
        println!("module context, read before writing failure modes: {file}");
    }
    match hits.len() {
        0 => println!("no spec covers these paths"),
        count => {
            let ids: Vec<&str> = hits.iter().map(|(entry, _)| entry.spec.id.as_str()).collect();
            println!(
                "{count} spec{} impacted; baseline before changing code: skies proof verify {}",
                if count == 1 { "" } else { "s" },
                ids.join(" ")
            );
        }
    }
    Ok(0)
}

/// A few names and a count, so a wide diff stays readable.
fn abbreviate(paths: &[&String]) -> String {
    const SHOWN: usize = 4;
    let mut text = paths
        .iter()
        .take(SHOWN)
        .map(|path| path.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    if paths.len() > SHOWN {
        text.push_str(&format!(", … ({} more)", paths.len() - SHOWN));
    }
    text
}

/// A path given on the command line (relative to where the user stands, or absolute), made relative to the project
/// root with forward slashes, the form footprints are keyed by. Resolved lexically so a file that was deleted still
/// maps to the specs that covered it.
fn project_path(root: &Path, cwd: &Path, given: &Path) -> String {
    let joined = cwd.join(given);
    let mut parts: Vec<Component> = Vec::new();
    for part in joined.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    let absolute: PathBuf = parts.iter().collect();
    let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let absolute = match (absolute.parent().map(std::fs::canonicalize), absolute.file_name()) {
        (Some(Ok(parent)), Some(name)) => parent.join(name),
        _ => absolute,
    };
    let rel = absolute.strip_prefix(&root).unwrap_or(&absolute);
    rel.components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_line_paths_become_project_paths() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("src/a")).unwrap();
        let cwd = root.path().join("src");
        assert_eq!(project_path(root.path(), &cwd, Path::new("a/B.cs")), "src/a/B.cs");
        assert_eq!(project_path(root.path(), &cwd, Path::new("../Other.cs")), "Other.cs");
        assert_eq!(project_path(root.path(), &cwd, &root.path().join("src/a")), "src/a");
    }

    #[test]
    fn a_spec_covers_its_footprint_globs_folder_and_directories_above_them() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join(".specs/0001-a");
        std::fs::create_dir_all(&dir).unwrap();
        let entry = Indexed {
            spec: SpecDir {
                name: "0001-a".into(),
                id: "0001".into(),
                path: dir,
            },
            doc: SpecDoc::default(),
            footprint: ["src/Pay/Charge.cs".to_string()].into(),
            touches: {
                let mut builder = GlobSetBuilder::new();
                builder.add(Glob::new("src/Money/**").unwrap());
                builder.build().unwrap()
            },
        };
        assert!(entry.covers("src/Pay/Charge.cs"));
        assert!(entry.covers("src/Pay"));
        assert!(entry.covers("src/Money/Amount.cs"));
        assert!(entry.covers(".specs/0001-a/e2e/x.cs"));
        assert!(!entry.covers("src/Other.cs"));
        assert!(!entry.covers("src/Pa"));
    }
}
