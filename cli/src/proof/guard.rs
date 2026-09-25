//! What `record` refuses before it trusts a red or a green: a red that differs from green in its tests rather than
//! in the feature, and a green that is not the commit the receipt names.
//!
//! Red must differ from green in the feature alone. So a red.patch may not touch `.specs/` (it could rewrite the
//! spec's own cases, which red copies from the working tree before applying the patch) nor the files that run the
//! tests: the declared tests project, a file the runner's commands name, a file such a config names (a vitest
//! `setupFiles`), and the usual test configs (`vitest.config.*`, `playwright.config.*`, `*.Tests.csproj`, …). An
//! explicit `--red <rev>` whose diff to green touches those, or the shared files of `.specs/` (a stand-in backend,
//! a tsconfig), is refused the same way; the default red (the merge-base) only earns a warning, since the branch's
//! own diff is what review reads. Green must be HEAD: `record` refuses a working tree with other changes than the
//! specs' receipts, evidence, and red.patch files, unless `--allow-dirty` records `green.dirty: true`.
//!
//! What no check can see is a test that tells red from green by where it runs (`$PWD`, a path, the git state): the
//! receipt proves the cases failed there and passed here, and review of the cases is what rules that out.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use anyhow::{Result, bail};
use regex::Regex;

use super::git::Repo;
use super::spec::{EVIDENCE_DIR, RECEIPT_FILE, RED_PATCH_FILE, SPECS_DIR, SpecDir};
use crate::manifest::{Project, Runner};

/// Test configs recognized by name wherever they sit, beside what the manifest and the runner name.
static TEST_CONFIG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"^(?:(?:vitest|vite|playwright|jest)\.config|vitest\.(?:workspace|setup)|jest\.setup|setupTests)\.[a-z]+$",
        r"|^[^/]*Tests?\.[cf]sproj$",
        r"|^(?:dart_test\.yaml|flutter_test_config\.dart)$",
    ))
    .expect("valid regex")
});

/// A quoted relative or absolute path in a config file (`setupFiles: ["./test/setup.ts"]`).
static QUOTED_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"["'`]((?:\.{1,2}/|/)?[A-Za-z0-9_.@/-]+\.[A-Za-z0-9]+)["'`]"#).expect("valid regex"));

/// The files that run a runner's tests, relative to the repository top.
pub struct TestFiles {
    files: BTreeSet<String>,
    dirs: Vec<String>,
    prefix: String,
}

impl TestFiles {
    /// The tests projects the manifest declares, the files the runner's commands name, and the files those name.
    pub fn of(project: &Project, repo: &Repo, runner: &Runner) -> TestFiles {
        let top = std::fs::canonicalize(&repo.top).unwrap_or_else(|_| repo.top.clone());
        let relative = |path: &Path| -> Option<String> {
            let path = std::fs::canonicalize(path).ok()?;
            let rel = path.strip_prefix(&top).ok()?;
            Some(
                rel.components()
                    .map(|part| part.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/"),
            )
        };
        let dirs = project
            .manifest
            .products
            .values()
            .filter_map(|product| product.tests.as_deref())
            .filter_map(|dir| relative(&project.root.join(dir)))
            .map(|dir| format!("{dir}/"))
            .collect();
        let mut files = BTreeSet::new();
        let commands = [
            runner.setup.as_deref(),
            runner.build.as_deref(),
            Some(runner.command.as_str()),
        ];
        for word in commands.into_iter().flatten().flat_map(words) {
            let path = project.root.join(&word);
            if !path.is_file() {
                continue;
            }
            if let Some(rel) = relative(&path) {
                files.insert(rel);
            }
            if !is_config(&path) {
                continue;
            }
            // A config names its setup files (vitest's `setupFiles`), which run before every case; the other paths
            // it names (aliases to the app's own source) are the code under test, not what runs it.
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
            for named in QUOTED_PATH
                .captures_iter(&text)
                .map(|capture| PathBuf::from(&capture[1]))
            {
                let named = if named.is_absolute() { named } else { dir.join(named) };
                if is_config(&named)
                    && let Some(rel) = named.is_file().then(|| relative(&named)).flatten()
                {
                    files.insert(rel);
                }
            }
        }
        TestFiles {
            files,
            dirs,
            prefix: repo.prefix.clone(),
        }
    }

    /// Whether a path relative to the repository top runs the tests.
    fn holds(&self, path: &str) -> bool {
        let name = path.rsplit('/').next().unwrap_or(path);
        self.files.contains(path) || self.dirs.iter().any(|dir| path.starts_with(dir)) || TEST_CONFIG.is_match(name)
    }

    /// The paths of a red.patch that it may not touch: anything under `.specs/`, or a file that runs the tests. A
    /// patch path is relative to the project root or to the repository top, so both readings are checked.
    pub fn refused_in_patch(&self, paths: &[String]) -> Vec<String> {
        let specs = format!("{}{SPECS_DIR}/", self.prefix);
        paths
            .iter()
            .filter(|path| {
                let readings = [format!("{}{path}", self.prefix), path.to_string()];
                readings.iter().any(|path| path.starts_with(&specs) || self.holds(path))
            })
            .cloned()
            .collect()
    }

    /// The paths of a red→green diff (relative to the top) that make red differ in its tests: a file that runs them,
    /// or a shared file of `.specs/` outside any spec folder (a stand-in backend, a tsconfig, not the .gitignore).
    pub fn refused_in_diff(&self, paths: &[String]) -> Vec<String> {
        let specs = format!("{}{SPECS_DIR}/", self.prefix);
        paths
            .iter()
            .filter(|path| {
                let shared = path.strip_prefix(&specs).is_some_and(|rest| {
                    let first = rest.split('/').next().unwrap_or(rest);
                    let spec_folder = rest.contains('/')
                        && first
                            .split_once('-')
                            .is_some_and(|(id, _)| id.bytes().all(|b| b.is_ascii_digit()));
                    !spec_folder && rest != ".gitignore"
                });
                shared || self.holds(path)
            })
            .cloned()
            .collect()
    }
}

/// Whether a file is a test config or setup file by its name.
fn is_config(path: &Path) -> bool {
    let name = path.file_name().map(|name| name.to_string_lossy()).unwrap_or_default();
    TEST_CONFIG.is_match(&name) || name.to_ascii_lowercase().contains("setup")
}

/// The words of a shell command that could be paths: split on blanks, unquoted, and split again at `=` so
/// `--config=vitest.config.ts` counts. Words holding a placeholder are left out.
fn words(command: &str) -> Vec<String> {
    command
        .split_whitespace()
        .map(|word| word.trim_matches(|ch| matches!(ch, '\'' | '"')))
        .flat_map(|word| word.split('='))
        .filter(|word| !word.is_empty() && !word.contains('{'))
        .map(String::from)
        .collect()
}

/// Refuses a red.patch that touches `.specs/` or a file that runs the tests, before it is stored or applied.
pub fn check_patch(tests: &TestFiles, repo: &Repo, spec: &SpecDir, patch: &Path) -> Result<()> {
    let refused = tests.refused_in_patch(&repo.patch_paths(patch)?);
    if !refused.is_empty() {
        bail!(
            "{} touches {}. Red must differ from green in the feature alone: its cases come from the working tree \
             byte for byte, and so do the files that run them. Make the patch remove the feature's code only, and \
             keep {}/{RED_PATCH_FILE} out of it.",
            patch.display(),
            refused.join(", "),
            spec.rel()
        );
    }
    Ok(())
}

/// Refuses an explicit `--red` revision whose diff to green touches what runs the tests, and warns when the default
/// red does: the branch's own diff, which review reads, changed the tests' setup along with the feature.
pub fn check_red_diff(tests: &TestFiles, repo: &Repo, commit: &str, explicit: bool) -> Result<()> {
    let refused = tests.refused_in_diff(&repo.diff_top(commit)?);
    if refused.is_empty() {
        return Ok(());
    }
    let list = refused.join(", ");
    if explicit {
        bail!(
            "--red {}: between it and the working tree, {list} changed. Those run the tests, so red would differ \
             from green in how the cases run, not only in the feature. Pick a red revision after that change.",
            &commit[..commit.len().min(7)]
        );
    }
    eprintln!(
        "warning: since red, this branch also changed what runs the tests ({list}); a reviewer should check that red \
         fails because the feature is missing, not because of that change."
    );
    Ok(())
}

/// Whether green is a dirty working tree, refusing one unless `allow_dirty`: the receipt names green by its commit.
pub fn check_green(repo: &Repo, given_patch: Option<&Path>, allow_dirty: bool) -> Result<bool> {
    let dirty = dirty(repo, given_patch)?;
    if dirty.is_empty() {
        return Ok(false);
    }
    if allow_dirty {
        eprintln!(
            "warning: recording green from uncommitted changes ({} paths); the receipt says `\"dirty\": true`",
            dirty.len()
        );
        return Ok(true);
    }
    const SHOWN: usize = 10;
    let mut list: Vec<String> = dirty.iter().take(SHOWN).map(|path| format!("  {path}")).collect();
    if dirty.len() > SHOWN {
        list.push(format!("  … and {} more", dirty.len() - SHOWN));
    }
    bail!(
        "the working tree differs from HEAD, so green would not be the commit the receipt names:\n{}\nCommit the \
         spec, its e2e, and the code first (the receipt, evidence/, and red.patch may stay uncommitted), or pass \
         --allow-dirty to record anyway with `\"dirty\": true` under green.",
        list.join("\n")
    )
}

/// The paths that make the working tree differ from HEAD, relative to the repository top, other than what `record`
/// itself writes (any spec's receipt, evidence, and red.patch, so specs can be recorded one after another, and
/// `.specs/.gitignore`) and the patch it was given. None of those is an input to a green run.
fn dirty(repo: &Repo, given_patch: Option<&Path>) -> Result<Vec<String>> {
    let specs = format!("{}{SPECS_DIR}/", repo.prefix);
    let written = |path: &str| {
        path.strip_prefix(&specs)
            .is_some_and(|rest| match rest.split_once('/') {
                Some((_, inside)) => {
                    inside == RECEIPT_FILE
                        || inside == RED_PATCH_FILE
                        || inside.starts_with(&format!("{EVIDENCE_DIR}/"))
                }
                None => rest == ".gitignore",
            })
    };
    let top = std::fs::canonicalize(&repo.top).unwrap_or_else(|_| repo.top.clone());
    let given = given_patch
        .and_then(|patch| std::fs::canonicalize(patch).ok())
        .and_then(|patch| patch.strip_prefix(&top).ok().map(Path::to_path_buf))
        .map(|rel| {
            rel.components()
                .map(|part| part.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/")
        });
    Ok(repo
        .dirty()?
        .into_iter()
        .filter(|path| !written(path) && given.as_deref() != Some(path.as_str()))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files() -> TestFiles {
        TestFiles {
            files: BTreeSet::from(["frontend-sdk/vitest.config.ts".to_string()]),
            dirs: vec!["app/tests/App.Tests/".to_string()],
            prefix: "app/".to_string(),
        }
    }

    #[test]
    fn a_patch_may_not_touch_the_specs_or_what_runs_the_tests() {
        let refused = files().refused_in_patch(&[
            "src/Deposit.cs".into(),
            ".specs/0001-a/e2e/a.test.ts".into(),
            "app/.specs/web.setup.ts".into(),
            "tests/App.Tests/App.Tests.csproj".into(),
            "web/vite.config.ts".into(),
            "web/src/setupTests.ts".into(),
            "frontend-sdk/vitest.config.ts".into(),
        ]);
        assert_eq!(
            refused,
            [
                ".specs/0001-a/e2e/a.test.ts",
                "app/.specs/web.setup.ts",
                "tests/App.Tests/App.Tests.csproj",
                "web/vite.config.ts",
                "web/src/setupTests.ts",
                "frontend-sdk/vitest.config.ts",
            ]
        );
    }

    #[test]
    fn a_red_diff_may_change_spec_folders_but_not_shared_spec_files() {
        let refused = files().refused_in_diff(&[
            "app/.specs/0003-new/spec.md".into(),
            "app/.specs/0003-new/e2e/a.test.ts".into(),
            "app/.specs/.gitignore".into(),
            "app/.specs/web.setup.ts".into(),
            "app/.specs/helpers/seed.ts".into(),
            "app/src/Deposit.cs".into(),
            "app/tests/App.Tests/Setup.cs".into(),
        ]);
        assert_eq!(
            refused,
            [
                "app/.specs/web.setup.ts",
                "app/.specs/helpers/seed.ts",
                "app/tests/App.Tests/Setup.cs"
            ]
        );
    }

    #[test]
    fn command_words_that_could_be_paths() {
        assert_eq!(
            words("node a/vitest.mjs run --config=../x/vitest.config.ts --outputFile={report} '{dir}'"),
            [
                "node",
                "a/vitest.mjs",
                "run",
                "--config",
                "../x/vitest.config.ts",
                "--outputFile"
            ]
        );
    }
}
