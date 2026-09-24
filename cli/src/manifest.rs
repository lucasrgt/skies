//! `Skies.toml`: the application's topology and the runners that execute spec E2E.
//!
//! ```toml
//! [workspace]
//! name = "Hostpoint"
//!
//! [products.app]
//! backend = "src/Hostpoint.Api"
//! frontend = ["clients/web", "clients/hosts"]
//!
//! [runners.api]
//! command = "dotnet test tests/Hostpoint.Tests --filter FullyQualifiedName~Specs.S{id}. --logger trx;LogFileName={report}"
//! report = "tests/Hostpoint.Tests/TestResults/{id}.trx"
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
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Product {
    pub backend: Option<String>,
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
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Runner {
    pub command: String,
    /// Where the report lands, relative to the root. Defaults to a file the engine chooses and passes as `{report}`.
    pub report: Option<String>,
    /// Run once before the first spec that uses this runner, e.g. to start a database.
    pub setup: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

/// The loaded manifest and the directory that holds it.
pub struct Project {
    pub root: PathBuf,
    pub manifest: Manifest,
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
            frontend = ["clients/web", "clients/mobile"]

            [runners.api]
            command = "dotnet test --filter Specs.S{id}."
            "#,
        )
        .unwrap();

        let app = &manifest.products["app"];
        assert_eq!(app.backend.as_deref(), Some("src/Demo.Api"));
        assert_eq!(
            app.frontend.iter().collect::<Vec<_>>(),
            ["clients/web", "clients/mobile"]
        );
        assert!(manifest.runners["api"].report.is_none());
    }

    #[test]
    fn rejects_unknown_keys() {
        let error = toml::from_str::<Manifest>("[workspace]\nname = \"x\"\n[gate]\nmode = \"ci\"\n").unwrap_err();
        assert!(error.to_string().contains("gate"));
    }
}
