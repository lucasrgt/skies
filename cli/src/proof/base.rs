//! Where a feature starts: the default red of `proof record` and the diff base of `proof impact`.
//!
//! Without `--red` or a red.patch, the base is the merge-base of HEAD with the branch features fork from:
//! `[workspace] default_branch` from Skies.toml, else the remote's default branch (`origin/HEAD`), else a local
//! `main` or `master`. A branch's upstream is deliberately not consulted: whether it names the fork point or the
//! branch's own remote copy depends on how the branch was created, and `default_branch` says it once for everyone.
//!
//! The choice is always printed with how it was made and how many commits separate it from HEAD, and a base far
//! behind HEAD is flagged: a red many unrelated commits back fails for reasons that are not the feature.

use anyhow::{Context, Result};

use super::git::Repo;
use super::record::short;

/// More commits than this between the base and HEAD earn a warning: few features are that long.
pub const FAR: usize = 50;

/// A resolved base and how it was chosen, in words (`merge-base with origin/main, default branch from origin/HEAD`).
pub struct Base {
    pub commit: String,
    pub how: String,
    /// Whether the user named it (`--red`, `--diff <rev>`); an explicit base is never second-guessed.
    pub explicit: bool,
}

impl Base {
    pub fn explicit(repo: &Repo, rev: &str, flag: &str) -> Result<Base> {
        Ok(Base {
            commit: repo.resolve(rev)?,
            how: format!("{flag} {rev}"),
            explicit: true,
        })
    }

    /// The merge-base of HEAD with the branch features fork from, by the order in the module docs.
    pub fn default(repo: &Repo, configured: Option<&str>) -> Result<Base> {
        let (branch, why) = match configured {
            Some(name) => (configured_branch(repo, name)?, "default_branch in Skies.toml"),
            None => match repo.origin_head() {
                Some(origin) => (origin, "default branch from origin/HEAD"),
                None => (
                    ["main", "master"]
                        .into_iter()
                        .find(|name| repo.resolve(name).is_ok())
                        .map(String::from)
                        .context(
                            "no branch to fork from (no default_branch in Skies.toml, origin/HEAD, main, or \
                             master); pass --red <rev>",
                        )?,
                    "no origin/HEAD",
                ),
            },
        };
        let commit = repo
            .merge_base("HEAD", &branch)
            .with_context(|| format!("no merge-base with {branch}; pass --red <rev>"))?;
        Ok(Base {
            commit,
            how: format!("merge-base with {branch}, {why}"),
            explicit: false,
        })
    }

    /// `5e2f092 (merge-base with main, default branch from origin/HEAD; 116 commits before HEAD)`, and a warning
    /// when a base the user did not name is far behind HEAD.
    pub fn describe(&self, repo: &Repo) -> (String, Option<String>) {
        let behind = repo.count(&self.commit, "HEAD").ok();
        let distance = match behind {
            Some(0) | None => String::new(),
            Some(1) => "; 1 commit before HEAD".to_string(),
            Some(count) => format!("; {count} commits before HEAD"),
        };
        let line = format!("{} ({}{distance})", short(&self.commit), self.how);
        let warning = match behind {
            Some(count) if count > FAR && !self.explicit => Some(format!(
                "warning: the base is {count} commits before HEAD; if the feature started later, set \
                 `default_branch` under [workspace] in Skies.toml or pass the revision explicitly"
            )),
            _ => None,
        };
        (line, warning)
    }
}

/// A configured branch as given (`develop`), or its remote copy when there is no local one (`origin/develop`).
fn configured_branch(repo: &Repo, name: &str) -> Result<String> {
    if repo.resolve(name).is_ok() {
        return Ok(name.to_string());
    }
    let remote = format!("origin/{name}");
    repo.resolve(&remote)
        .map(|_| remote)
        .with_context(|| format!("default_branch '{name}' in Skies.toml is neither a branch nor origin/{name}"))
}
