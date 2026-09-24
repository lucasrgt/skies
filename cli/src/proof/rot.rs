//! Red rot: a retro-spec's red.patch that no longer applies.
//!
//! A spec written after its code proves red by applying a patch that removes the feature. `verify` never reruns red,
//! so when the code under the patch moves on, the patch silently stops applying and the receipt's red could no
//! longer be reproduced. `proof status` says so (`red-rotted`), by `git apply --check` against the working tree:
//! the same files status reads for staleness, and what `record` will apply to once they are committed.
//!
//! Status must stay in milliseconds, and a git process per patch is not. So each result is cached, keyed by the
//! blake3 of the patch and of every file it touches, in `<git dir>/skies/red-rot.json`: private to the checkout,
//! never committed, and found from the filesystem alone. A patch is checked again only when it or one of its files
//! changed; with everything cached, status runs no git at all.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::git::{self, Repo};
use super::hash;
use super::spec::{RED_PATCH_FILE, SpecDir};

#[derive(Default, Serialize, Deserialize)]
struct Cache {
    /// Spec folder → the key the check was made for and whether the patch applied.
    specs: BTreeMap<String, Checked>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Checked {
    key: String,
    applies: bool,
}

/// The specs among `specs` whose red.patch no longer applies to the working tree. Empty outside a git checkout.
pub fn rotted(root: &Path, specs: &[SpecDir]) -> Vec<String> {
    let Some(checkout) = Checkout::find(root) else {
        return Vec::new();
    };
    let path = checkout.git_dir.join("skies").join("red-rot.json");
    let cached: Cache = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    let mut fresh = Cache::default();
    let mut repo: Option<Option<Repo>> = None;
    let mut rotted = Vec::new();
    for spec in specs {
        let patch = spec.file(RED_PATCH_FILE);
        let Ok(text) = std::fs::read_to_string(&patch) else {
            continue;
        };
        let key = checkout.key(root, &text);
        let applies = match cached.specs.get(&spec.name).filter(|checked| checked.key == key) {
            Some(checked) => checked.applies,
            None => {
                let repo = repo.get_or_insert_with(|| Repo::open(root).ok());
                match repo {
                    Some(repo) => repo.applies(&patch),
                    None => continue,
                }
            }
        };
        if !applies {
            rotted.push(spec.name.clone());
        }
        fresh.specs.insert(spec.name.clone(), Checked { key, applies });
    }
    let changed = fresh.specs.len() != cached.specs.len()
        || fresh
            .specs
            .iter()
            .any(|(name, checked)| cached.specs.get(name).is_none_or(|old| old.key != checked.key));
    if changed && let Ok(text) = serde_json::to_string_pretty(&fresh) {
        // A cache that cannot be written only costs the next status a few git calls.
        let _ = std::fs::create_dir_all(path.parent().unwrap_or(&checkout.git_dir));
        let _ = std::fs::write(&path, text);
    }
    rotted
}

/// The checkout's top level and git directory, found by walking up from the project root. A linked worktree's
/// `.git` is a file naming its git directory, so each worktree keeps its own cache.
struct Checkout {
    top: PathBuf,
    git_dir: PathBuf,
    /// The project root relative to `top`, with a trailing slash, or empty at the top.
    prefix: String,
}

impl Checkout {
    fn find(root: &Path) -> Option<Checkout> {
        for dir in root.ancestors() {
            let dot_git = dir.join(".git");
            let git_dir = if dot_git.is_dir() {
                dot_git
            } else if dot_git.is_file() {
                let text = std::fs::read_to_string(&dot_git).ok()?;
                let named = PathBuf::from(text.strip_prefix("gitdir:")?.trim());
                if named.is_absolute() { named } else { dir.join(named) }
            } else {
                continue;
            };
            let prefix = hash::relative(dir, root);
            let prefix = if prefix.is_empty() {
                prefix
            } else {
                format!("{prefix}/")
            };
            return Some(Checkout {
                top: dir.to_path_buf(),
                git_dir,
                prefix,
            });
        }
        None
    }

    /// blake3 over the patch and every file it names, as it is on disk now. Paths are resolved the way
    /// [`Repo::patch_footprint`] reads them: from the repository top when every one carries the project prefix,
    /// else from the project root.
    fn key(&self, root: &Path, patch: &str) -> String {
        let paths = git::patch_files(patch);
        let from_top = !self.prefix.is_empty() && paths.iter().all(|path| path.starts_with(&self.prefix));
        let base = if from_top { &self.top } else { root };
        let mut hasher = blake3::Hasher::new();
        hasher.update(blake3::hash(patch.as_bytes()).as_bytes());
        for path in &paths {
            let file = hash::hash_file(&base.join(path)).unwrap_or_else(|| hash::ABSENT.to_string());
            hasher.update(path.as_bytes());
            hasher.update(b"\0");
            hasher.update(file.as_bytes());
            hasher.update(b"\n");
        }
        format!("blake3:{}", hasher.finalize().to_hex())
    }
}
