//! The few git operations `record` and `impact` need, through the git CLI.
//!
//! Shelling out keeps the binary free of libgit2 and behaves exactly like the git the author already uses
//! (config, worktrees, sparse checkouts). `run` never calls it.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

/// A git repository seen from a project root that may sit below the repository's top level (a monorepo app,
/// or the framework's own sample).
pub struct Repo {
    pub top: PathBuf,
    /// The project root relative to `top`, with forward slashes and a trailing slash, or empty at the top.
    pub prefix: String,
    root: PathBuf,
}

impl Repo {
    pub fn open(root: &Path) -> Result<Repo> {
        let top = PathBuf::from(
            git(root, &["rev-parse", "--show-toplevel"]).context("the project root is not inside a git repository")?,
        );
        let prefix = git(root, &["rev-parse", "--show-prefix"])?;
        Ok(Repo {
            top,
            prefix,
            root: root.to_path_buf(),
        })
    }

    pub fn run(&self, args: &[&str]) -> Result<String> {
        git(&self.root, args)
    }

    pub fn resolve(&self, rev: &str) -> Result<String> {
        self.run(&["rev-parse", "--verify", "--quiet", &format!("{rev}^{{commit}}")])
            .with_context(|| format!("'{rev}' is not a commit in this repository"))
    }

    pub fn head(&self) -> Result<String> {
        self.resolve("HEAD")
    }

    /// The remote's default branch (`origin/main`), from `origin/HEAD`.
    pub fn origin_head(&self) -> Option<String> {
        self.run(&["symbolic-ref", "--quiet", "--short", "refs/remotes/origin/HEAD"])
            .ok()
    }

    pub fn merge_base(&self, a: &str, b: &str) -> Result<String> {
        self.run(&["merge-base", a, b])
            .with_context(|| format!("{a} shares no history with {b}"))
    }

    /// How many commits `to` has that `from` does not (`git rev-list --count from..to`).
    pub fn count(&self, from: &str, to: &str) -> Result<usize> {
        let text = self.run(&["rev-list", "--count", &format!("{from}..{to}")])?;
        text.parse()
            .with_context(|| format!("git rev-list --count printed '{text}'"))
    }

    /// Files that differ between `rev` and the working tree, plus untracked files, relative to the project root.
    /// Paths outside the project root are left out: a receipt describes one application.
    pub fn changed_since(&self, rev: &str) -> Result<Vec<String>> {
        let mut paths = lines(&self.run(&["diff", "--name-only", "--relative", "--no-renames", rev])?);
        paths.extend(lines(&self.run(&["ls-files", "--others", "--exclude-standard"])?));
        Ok(paths)
    }

    /// Every path of the repository that differs from HEAD or is untracked (ignored files left out), relative to the
    /// repository top: what makes the working tree something other than the commit a receipt names as green.
    pub fn dirty(&self) -> Result<Vec<String>> {
        let text = git_raw(&self.top, &["status", "--porcelain=v1", "-z", "--untracked-files=all"])?;
        let mut paths = Vec::new();
        let mut entries = text.split('\0').filter(|entry| !entry.is_empty());
        while let Some(entry) = entries.next() {
            let (status, path) = entry.split_at(entry.len().min(3));
            paths.push(path.to_string());
            // A rename or copy is followed by its source path, which changed too.
            if status.starts_with(['R', 'C']) || status[1..].starts_with(['R', 'C']) {
                paths.extend(entries.next().map(String::from));
            }
        }
        Ok(paths)
    }

    /// The files that differ between `rev` and the working tree, relative to the repository top.
    pub fn diff_top(&self, rev: &str) -> Result<Vec<String>> {
        let text = git_raw(&self.top, &["diff", "--name-only", "--no-renames", "-z", rev])?;
        Ok(text
            .split('\0')
            .filter(|path| !path.is_empty())
            .map(String::from)
            .collect())
    }

    /// The paths a patch touches, as written in it (after the `a/` or `b/` prefix), both sides of a rename.
    pub fn patch_paths(&self, patch: &Path) -> Result<Vec<String>> {
        let patch_text = patch.to_str().context("the patch path is not UTF-8")?;
        let text = git_raw(&self.top, &["apply", "--numstat", "-z", patch_text])
            .with_context(|| format!("{} is not a patch git can read", patch.display()))?;
        // `added\tdeleted\tpath\0`, or for a rename `added\tdeleted\t\0from\0to\0`.
        Ok(text
            .split('\0')
            .map(|field| field.rsplit('\t').next().unwrap_or(field))
            .filter(|path| !path.is_empty())
            .map(String::from)
            .collect())
    }

    /// Checks out `commit` in a fresh detached worktree inside the repository, at `<top>/.skies-red/<random>/checkout`.
    /// Inside, not under the system temp directory, so every configuration a tool looks up in the parent directories
    /// (a NuGet.config, .npmrc, global.json, or tool manifest at or above the repository) applies to red exactly as it
    /// does to the working tree. Not inside `.git/`, because tools refuse to read there (Vite, and so vitest, denies
    /// `**/.git/**`). The folder is listed in the repository's local `info/exclude`, so git never shows it, and it
    /// exists only while red runs: the returned guard removes the worktree and the folder on drop, so an early return
    /// or a failing runner leaves nothing behind.
    pub fn temp_worktree(&self, commit: &str) -> Result<TempWorktree> {
        // A worktree left by a killed process would otherwise block nothing but clutter `git worktree list`.
        let _ = self.run(&["worktree", "prune"]);
        self.exclude(RED_DIR)?;
        let parent = self.top.join(RED_DIR);
        std::fs::create_dir_all(&parent).with_context(|| format!("creating {}", parent.display()))?;
        let dir = tempfile::Builder::new()
            .prefix("red-")
            .tempdir_in(&parent)
            .with_context(|| format!("creating a directory under {}", parent.display()))?;
        let path = dir.path().join("checkout");
        let path_text = path.to_str().context("the temporary directory path is not UTF-8")?;
        git(
            &self.top,
            &["worktree", "add", "--detach", "--quiet", path_text, commit],
        )
        .with_context(|| format!("creating a worktree at {commit}"))?;
        Ok(TempWorktree {
            top: self.top.clone(),
            path,
            dir: Some(dir),
            parent,
        })
    }

    /// Adds `/<name>/` to the repository's local exclude file (`info/exclude`, never committed) when it is missing,
    /// so a folder the engine creates at the top never shows as untracked, nor as an undeclared root entry.
    fn exclude(&self, name: &str) -> Result<()> {
        let common = PathBuf::from(self.run(&["rev-parse", "--path-format=absolute", "--git-common-dir"])?);
        let path = common.join("info").join("exclude");
        let rule = format!("/{name}/");
        let mut text = std::fs::read_to_string(&path).unwrap_or_default();
        if text.lines().any(|line| line.trim() == rule) {
            return Ok(());
        }
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&format!("# skies proof record: red's temporary checkout\n{rule}\n"));
        std::fs::create_dir_all(common.join("info"))?;
        std::fs::write(&path, text).with_context(|| format!("writing {}", path.display()))
    }

    /// Applies a patch whose paths are relative to the project root (what `git diff --relative` writes from the app)
    /// or, failing that, to the repository top (what a plain `git diff` writes), so both habits just work.
    pub fn apply(&self, checkout_top: &Path, patch: &Path) -> Result<()> {
        let patch_text = patch.to_str().context("the patch path is not UTF-8")?;
        let directory = format!("--directory={}", self.prefix.trim_end_matches('/'));
        let mut styles: Vec<Vec<&str>> = Vec::new();
        if !self.prefix.is_empty() {
            styles.push(vec!["apply", "--whitespace=nowarn", &directory]);
        }
        styles.push(vec!["apply", "--whitespace=nowarn"]);
        let mut last_error = None;
        for style in styles {
            let check: Vec<&str> = style.iter().copied().chain(["--check", patch_text]).collect();
            if let Err(error) = git(checkout_top, &check) {
                last_error = Some(error);
                continue;
            }
            let apply: Vec<&str> = style.iter().copied().chain([patch_text]).collect();
            return git(checkout_top, &apply).map(|_| ());
        }
        Err(last_error.expect("at least one patch style"))
            .with_context(|| format!("{} does not apply", patch.display()))
    }
}

/// The folder at the repository top that holds red's checkouts while they run.
pub const RED_DIR: &str = ".skies-red";

pub struct TempWorktree {
    top: PathBuf,
    /// The checkout's top level, the counterpart of [`Repo::top`].
    pub path: PathBuf,
    dir: Option<tempfile::TempDir>,
    parent: PathBuf,
}

impl Drop for TempWorktree {
    fn drop(&mut self) {
        if let Some(path) = self.path.to_str() {
            let _ = git(&self.top, &["worktree", "remove", "--force", path]);
        }
        let _ = git(&self.top, &["worktree", "prune"]);
        drop(self.dir.take());
        // Empty unless another record runs at the same time, whose checkout must stay.
        let _ = std::fs::remove_dir(&self.parent);
    }
}

fn git(dir: &Path, args: &[&str]) -> Result<String> {
    git_raw(dir, args).map(|text| text.trim().to_string())
}

/// Git's output as printed, for `-z` output whose fields may end in spaces.
fn git_raw(dir: &Path, args: &[&str]) -> Result<String> {
    // quotepath=off keeps non-ASCII paths literal, so they match `touches` and module folders like any other path.
    let output = Command::new("git")
        .args(["-c", "core.quotepath=off"])
        .args(args)
        .current_dir(dir)
        .output()
        .context("running git (is it installed and on PATH?)")?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(String::from)
        .collect()
}
