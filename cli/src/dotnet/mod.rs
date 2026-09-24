//! .NET scaffolders: `skies new` and the backend `skies g` generators.
//!
//! Every generator emits plain C# that is doctor-clean by construction and edits the owner's files only at known
//! anchors, printing a precise manual step when an anchor is missing instead of failing. They are ports of the
//! 4.x C# CLI with the proof ceremony removed: no generated unit tests, journeys, `[AVP]` proofs, or
//! `*.spec.toml`. The one exception is the auth family, whose blueprint ships its tests as a spec
//! (`.specs/<id>-auth*/`), because auth is the feature most worth proving and least worth rewriting per app.
//!
//! Generators run from the API project directory (the one holding `<App>.Api.csproj`). A user error prints
//! `skies: ...` and exits 1, like the 4.x CLI; `Err` is reserved for I/O failures.

mod app;
mod auth;
mod blueprint;
mod crud;
mod embedded;
mod error_codes;
mod flow_specs;
mod flows;
mod scaffold;
mod specs;
mod text;

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use crate::Generate;

/// The Skies.Framework.* package version that generators stamp into csproj files. It moves with the lockstep
/// release; the `skies new` template carries the same literal.
pub const FRAMEWORK_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn new_app(name: &str) -> Result<u8> {
    app::new_app(&std::env::current_dir()?, name)
}

pub fn generate(command: Generate) -> Result<u8> {
    let root = std::env::current_dir()?;
    match command {
        Generate::Module { name } => scaffold::module(&root, &name),
        Generate::Slice { module, name } => scaffold::slice(&root, &module, &name),
        Generate::Entity { module, name } => scaffold::entity(&root, &module, &name),
        Generate::Vo { name } => scaffold::value_object(&root, &name),
        Generate::Crud { module, entity } => crud::generate(&root, &module, &entity),
        Generate::Hub { module, name } => scaffold::hub(&root, &module, &name),
        Generate::Auth {
            skip_tenancy,
            skip_cookies,
        } => auth::generate(&root, !skip_tenancy, !skip_cookies),
        Generate::AuthOtp => flows::generate(&root, flow_specs::Flow::Otp),
        Generate::AuthOauth => flows::generate(&root, flow_specs::Flow::OAuth),
        Generate::AuthEmail => flows::generate(&root, flow_specs::Flow::Email),
        Generate::Feature { .. } | Generate::Client { .. } | Generate::FlutterApp { .. } => {
            bail!("not a .NET generator")
        }
    }
}

/// The application project a generator runs in: the directory, its csproj, and the root namespace.
pub(crate) struct ApiProject {
    pub root: PathBuf,
    pub csproj: PathBuf,
    /// The csproj file name without extension, e.g. `Acme.Api`: the root namespace by .NET convention.
    pub namespace: String,
}

impl ApiProject {
    /// Finds the csproj in `root`, or prints the 4.x guidance and returns `None`.
    pub fn open(root: &Path) -> Result<Option<ApiProject>> {
        let mut projects: Vec<PathBuf> = std::fs::read_dir(root)?
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "csproj"))
            .collect();
        projects.sort();
        let Some(csproj) = projects.into_iter().next() else {
            eprintln!("skies: no .csproj here — run this from the application project directory.");
            return Ok(None);
        };
        let namespace = csproj.file_stem().unwrap_or_default().to_string_lossy().into_owned();
        Ok(Some(ApiProject {
            root: root.to_path_buf(),
            csproj,
            namespace,
        }))
    }

    /// The app name: the namespace without its `.Api` suffix (`Acme.Api` becomes `Acme`).
    pub fn app_name(&self) -> &str {
        self.namespace.strip_suffix(".Api").unwrap_or(&self.namespace)
    }

    /// The lowercase app name, used for cookie names, JWT issuer/audience, and database names.
    pub fn app_lower(&self) -> String {
        self.app_name().to_lowercase()
    }

    /// The solution root by the Skies layout: the API project sits at `src/<App>.Api`.
    pub fn solution_root(&self) -> PathBuf {
        match self.root.parent().and_then(Path::parent) {
            Some(root) => root.to_path_buf(),
            None => self.root.join("..").join(".."),
        }
    }

    /// `tests/<App>.Tests` under the solution root, where the test project lives by convention.
    pub fn test_dir(&self) -> PathBuf {
        self.solution_root()
            .join("tests")
            .join(format!("{}.Tests", self.app_name()))
    }

    pub fn module_dir(&self, module: &str) -> PathBuf {
        self.root.join("Modules").join(module)
    }
}

/// The first `*.csproj` in `dir`, if the directory exists.
pub(crate) fn first_csproj(dir: &Path) -> Option<PathBuf> {
    let mut projects: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "csproj"))
        .collect();
    projects.sort();
    projects.into_iter().next()
}
