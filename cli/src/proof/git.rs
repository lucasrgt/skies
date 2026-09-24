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

    /// The branch HEAD is on, or `None` when detached.
    pub fn current_branch(&self) -> Option<String> {
        self.run(&["symbolic-ref", "--quiet", "--short", "HEAD"]).ok()
    }

    /// The current branch's upstream as git abbreviates it (`origin/v5`), if it has one.
    pub fn upstream(&self) -> Option<String> {
        self.run(&["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}"])
            .ok()
            .filter(|name| !name.is_empty())
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

    /// Checks out `commit` in a fresh detached worktree under the system temp directory. The returned guard removes
    /// it on drop, so an early return or a failing runner never leaves a stray worktree behind.
    pub fn temp_worktree(&self, commit: &str) -> Result<TempWorktree> {
        // A worktree left by a killed process would otherwise block nothing but clutter `git worktree list`.
        let _ = self.run(&["worktree", "prune"]);
        let dir = tempfile::Builder::new()
            .prefix("skies-red-")
            .tempdir()
            .context("creating a temporary directory")?;
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
            _dir: dir,
        })
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

pub struct TempWorktree {
    top: PathBuf,
    /// The checkout's top level, the counterpart of [`Repo::top`].
    pub path: PathBuf,
    _dir: tempfile::TempDir,
}

impl Drop for TempWorktree {
    fn drop(&mut self) {
        if let Some(path) = self.path.to_str() {
            let _ = git(&self.top, &["worktree", "remove", "--force", path]);
        }
        let _ = git(&self.top, &["worktree", "prune"]);
    }
}

fn git(dir: &Path, args: &[&str]) -> Result<String> {
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
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(String::from)
        .collect()
}
