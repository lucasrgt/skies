//! `Skies.toml`: the application's topology and the runners that execute spec E2E.
//!
//! ```toml
//! [workspace]
//! name = "Hostpoint"
//! default_branch = "develop"   # optional: where features branch from, for red and `proof impact`
//! root = ["src/", "tests/", "clients/", "*.slnx", "README.md"]   # everything allowed at the repository root
//!
//! [products.app]
//! backend = "src/Hostpoint.Api"
//! tests = "tests/Hostpoint.Tests"
//! frontend = ["clients/web", "clients/hosts"]
//!
//! [runners.api]
//! scope = ["src/"]
//! command = "dotnet test tests/Hostpoint.Tests --filter FullyQualifiedName~Specs.S{id}. --logger trx;LogFileName={report} --collect \"XPlat Code Coverage\" --results-directory {coverage} -- DataCollectionRunSettings.DataCollectors.DataCollector.Configuration.Format=cobertura"
//!
//! [runners.web]
//! scope = ["clients/web/"]
//! command = "npx vitest run {dir} --reporter=junit --outputFile={report} --coverage.enabled --coverage.reporter=lcov --coverage.reportsDirectory={coverage}"
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

pub const FILE_NAME: &str = "Skies.toml";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub workspace: Workspace,
    #[serde(default)]
    pub products: BTreeMap<String, Product>,
    #[serde(default)]
    pub runners: BTreeMap<String, Runner>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Workspace {
    pub name: String,
    /// The branch features fork from. `proof record` takes red as the merge-base of HEAD with it, and `proof impact`
    /// diffs from there, ahead of the current branch's upstream and `origin/HEAD`. Set it when work happens on a
    /// long-lived branch other than the remote's default (a `develop`, a major-version branch).
    pub default_branch: Option<String>,
    /// Everything allowed at the repository root: globs matched against one root entry's name, a trailing `/` for a
    /// directory and none for a file (`.git` and `Skies.toml` are implicit). `skies doctor` flags any other root entry
    /// (SKYWS001), so junk has to be moved, deleted, or declared on purpose. Required: a manifest without it gets one
    /// finding with the list to paste. See `doctor::workspace` for the matching rules.
    pub root: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Product {
    pub backend: Option<String>,
    /// The .NET tests project that compiles the spec E2E. When declared, `skies doctor` builds it instead of the
    /// backend: it references the backend, so one build runs the SKY analyzers over both, and the tests project is
    /// where SKY0029 sees a test that lives outside `.specs/`.
    pub tests: Option<String>,
    #[serde(default)]
    pub frontend: Paths,
}

/// One path or a list of paths, so a product with a single frontend stays a one-liner.
#[derive(Debug, Default, Deserialize)]
#[serde(untagged)]
pub enum Paths {
    #[default]
    None,
    One(String),
    Many(Vec<String>),
}

impl Paths {
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        let slice: &[String] = match self {
            Paths::None => &[],
            Paths::One(one) => std::slice::from_ref(one),
            Paths::Many(many) => many,
        };
        slice.iter().map(String::as_str)
    }
}

/// A shell command that runs one spec's E2E and writes a JUnit or TRX report.
///
/// Placeholders: `{id}` (the spec id, e.g. `0012`), `{dir}` (the spec's e2e folder, relative to the root),
/// `{spec}` (the spec folder name), `{report}` (the absolute report path the engine reads back), and `{evidence}`
/// (an absolute folder for screenshots and logs that end up in the spec's evidence/). The command runs through the
/// platform shell with the checkout's project root as its working directory; its exit code is not interpreted,
/// only the report is.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Runner {
    pub command: String,
    /// Where the report lands, relative to the root. Defaults to a file the engine chooses and passes as `{report}`.
    pub report: Option<String>,
    /// Where the runner writes coverage (a Cobertura or LCOV file, or a folder holding them), relative to the root.
    /// Defaults to a path the engine chooses and passes as `{coverage}` and `SKIES_COVERAGE`. Optional: a runner
    /// without coverage gets a footprint of the files changed since red.
    pub coverage: Option<String>,
    /// Run once before the first spec that uses this runner, e.g. to start a database.
    pub setup: Option<String>,
    /// Compiles the tests once per checkout, before the first spec that uses this runner (after `setup`), so
    /// `command` can skip building (`dotnet test --no-build`) and the specs one invocation reruns build once. A
    /// failing build means the tests did not build: on red every failure mode counts as failing.
    pub build: Option<String>,
    /// The paths (relative to the root; folders or files) this runner's specs exercise. Only files under them count
    /// for its specs: the diff part of a footprint, `touches` matches, coverage, and ctx notes. Without it, every
    /// file in the project does.
    pub scope: Option<Vec<String>>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

/// The loaded manifest and the directory that holds it.
pub struct Project {
    pub root: PathBuf,
    pub manifest: Manifest,
}

impl Runner {
    /// Whether `path` (relative to the root, forward slashes) belongs to this runner's specs.
    pub fn in_scope(&self, path: &str) -> bool {
        self.scope.as_ref().is_none_or(|scope| {
            scope.iter().any(|entry| {
                let entry = entry.trim_start_matches("./").trim_end_matches('/');
                entry.is_empty() || path == entry || path.strip_prefix(entry).is_some_and(|rest| rest.starts_with('/'))
            })
        })
    }
}

impl Project {
    /// Finds `Skies.toml` in `start` or its nearest ancestor.
    pub fn discover(start: &Path) -> Result<Project> {
        let mut dir = Some(start);
        while let Some(candidate) = dir {
            let path = candidate.join(FILE_NAME);
            if path.is_file() {
                return Ok(Project {
                    root: candidate.to_path_buf(),
                    manifest: load(&path)?,
                });
            }
            dir = candidate.parent();
        }
        bail!("no {FILE_NAME} in {} or any parent directory", start.display())
    }

    pub fn from_cwd() -> Result<Project> {
        Project::discover(&std::env::current_dir()?)
    }

    pub fn runner(&self, name: &str) -> Result<&Runner> {
        self.manifest.runners.get(name).with_context(|| {
            let known: Vec<&str> = self.manifest.runners.keys().map(String::as_str).collect();
            format!(
                "runner '{name}' is not declared in {FILE_NAME} (declared: {})",
                known.join(", ")
            )
        })
    }
}

pub fn load(path: &Path) -> Result<Manifest> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_products_and_runners() {
        let manifest: Manifest = toml::from_str(
            r#"
            [workspace]
            name = "Demo"

            [products.app]
            backend = "src/Demo.Api"
            tests = "tests/Demo.Tests"
            frontend = ["clients/web", "clients/mobile"]

            [runners.api]
            command = "dotnet test --filter Specs.S{id}."
            "#,
        )
        .unwrap();

        let app = &manifest.products["app"];
        assert_eq!(app.backend.as_deref(), Some("src/Demo.Api"));
        assert_eq!(app.tests.as_deref(), Some("tests/Demo.Tests"));
        assert_eq!(
            app.frontend.iter().collect::<Vec<_>>(),
            ["clients/web", "clients/mobile"]
        );
        assert!(manifest.runners["api"].report.is_none());
        assert!(manifest.workspace.default_branch.is_none());
        assert!(manifest.runners["api"].in_scope("anything/at/all.cs"));
    }

    #[test]
    fn reads_the_default_branch_and_a_runner_scope() {
        let manifest: Manifest = toml::from_str(
            r#"
            [workspace]
            name = "Demo"
            default_branch = "v5"
            root = ["frontend/", "*.slnx"]

            [runners.web]
            command = "npx vitest run {dir}"
            build = "npx tsc -b"
            scope = ["frontend/", "./shared/Contracts.ts"]
            "#,
        )
        .unwrap();
        assert_eq!(manifest.workspace.default_branch.as_deref(), Some("v5"));
        assert_eq!(
            manifest.workspace.root.as_deref(),
            Some(&["frontend/".to_string(), "*.slnx".to_string()][..])
        );
        let web = &manifest.runners["web"];
        assert_eq!(web.build.as_deref(), Some("npx tsc -b"));
        assert!(web.in_scope("frontend/web/src/App.tsx"));
        assert!(web.in_scope("shared/Contracts.ts"));
        assert!(
            !web.in_scope("frontend-sdk/index.ts"),
            "an entry is a path, not a string prefix"
        );
        assert!(!web.in_scope("backend/Api/Transfer.cs"));
        assert!(!web.in_scope("shared/Contracts.tsx"));
    }

    #[test]
    fn rejects_unknown_keys() {
        let error = toml::from_str::<Manifest>("[workspace]\nname = \"x\"\n[gate]\nmode = \"ci\"\n").unwrap_err();
        assert!(error.to_string().contains("gate"));
    }
}
