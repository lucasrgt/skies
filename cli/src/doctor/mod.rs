//! `skies doctor`: runs each platform's architecture analyzers and reports them together.
//!
//! Every platform already owns its checks: the SKY Roslyn analyzers run inside `dotnet build`, the SKYFE rules
//! inside the package's `npm run lint`, and the SKYFL rules natively here. The doctor only finds the packages
//! `Skies.toml` declares, runs one leg per package in parallel, and prints one table. It is run on purpose; no
//! hook calls it, and it polices nothing but architecture.
//!
//! Exit codes: 0 when no leg has an error-level finding, 1 when one does, 2 when a leg could not run at all
//! (a missing tool, a package without a manifest). Warnings are reported but never fail the run.

mod legs;
mod report;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::manifest::Project;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Error,
    Warning,
}

/// One diagnostic from any leg, normalized so the report can group and sort them the same way.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Finding {
    pub file: PathBuf,
    pub line: Option<usize>,
    pub code: String,
    pub severity: Severity,
    pub message: String,
}

impl Finding {
    pub fn new(code: &str, severity: Severity, file: PathBuf, line: Option<usize>, message: String) -> Finding {
        Finding {
            file,
            line,
            code: code.to_string(),
            severity,
            message,
        }
    }
}

/// What a leg is asked to check.
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    Dotnet(PathBuf),
    Eslint(PathBuf),
    Flutter(PathBuf),
    /// A declared frontend with neither `package.json` nor `pubspec.yaml`: reported, never silently skipped.
    Unknown(PathBuf),
}

/// How a leg ended.
#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    /// The analyzer ran; findings (possibly none) are its verdict.
    Ran,
    /// The leg does not apply (a React package without a `lint` script).
    Skipped(String),
    /// The analyzer could not produce a verdict: the tool is missing or the run failed without diagnostics.
    Failed(String),
}

pub struct Leg {
    pub name: String,
    pub status: Status,
    pub findings: Vec<Finding>,
    pub duration: Duration,
}

impl Leg {
    pub fn errors(&self) -> usize {
        self.findings.iter().filter(|f| f.severity == Severity::Error).count()
    }
}

pub fn run(build_args: &[String]) -> Result<u8> {
    let project = Project::from_cwd()?;
    let targets = targets(&project);
    if targets.is_empty() {
        println!(
            "skies doctor: {} ({}) declares no backend or frontend packages",
            project.manifest.workspace.name,
            crate::manifest::FILE_NAME
        );
        return Ok(0);
    }
    println!("skies doctor: {}", project.manifest.workspace.name);
    let started = Instant::now();
    let legs = run_legs(&project.root, &targets, build_args);
    print!("{}", report::render(&project.root, &legs, started.elapsed()));
    Ok(exit_code(&legs))
}

/// One target per declared package, in manifest order (products are sorted by name).
pub fn targets(project: &Project) -> Vec<Target> {
    let mut out = Vec::new();
    for product in project.manifest.products.values() {
        if let Some(backend) = &product.backend {
            out.push(Target::Dotnet(project.root.join(backend)));
        }
        for frontend in product.frontend.iter() {
            out.push(classify(&project.root.join(frontend)));
        }
    }
    out
}

fn classify(dir: &Path) -> Target {
    if dir.join("pubspec.yaml").is_file() {
        Target::Flutter(dir.to_path_buf())
    } else if dir.join("package.json").is_file() {
        Target::Eslint(dir.to_path_buf())
    } else {
        Target::Unknown(dir.to_path_buf())
    }
}

/// Runs every leg on its own thread. The legs are dominated by child processes (dotnet, npm), so plain threads
/// are the right tool; the Flutter leg parallelizes its own files with rayon.
pub fn run_legs(root: &Path, targets: &[Target], build_args: &[String]) -> Vec<Leg> {
    std::thread::scope(|scope| {
        let handles: Vec<_> = targets
            .iter()
            .map(|target| scope.spawn(move || legs::run(root, target, build_args)))
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("a doctor leg panicked"))
            .collect()
    })
}

pub fn exit_code(legs: &[Leg]) -> u8 {
    if legs.iter().any(|leg| matches!(leg.status, Status::Failed(_))) {
        2
    } else if legs.iter().any(|leg| leg.errors() > 0) {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leg(status: Status, severities: &[Severity]) -> Leg {
        let findings = severities
            .iter()
            .map(|s| Finding::new("SKY0001", *s, PathBuf::from("a.cs"), Some(1), "m".into()))
            .collect();
        Leg {
            name: "x".into(),
            status,
            findings,
            duration: Duration::ZERO,
        }
    }

    #[test]
    fn exit_code_separates_findings_from_legs_that_could_not_run() {
        assert_eq!(
            exit_code(&[leg(Status::Ran, &[]), leg(Status::Skipped("no lint".into()), &[])]),
            0
        );
        assert_eq!(exit_code(&[leg(Status::Ran, &[Severity::Warning])]), 0);
        assert_eq!(exit_code(&[leg(Status::Ran, &[Severity::Error])]), 1);
        assert_eq!(
            exit_code(&[
                leg(Status::Ran, &[Severity::Error]),
                leg(Status::Failed("x".into()), &[])
            ]),
            2
        );
    }

    #[test]
    fn targets_follow_the_manifest_and_classify_frontends_by_manifest_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("Skies.toml"),
            "[workspace]\nname = \"d\"\n[products.app]\nbackend = \"api\"\nfrontend = [\"web\", \"mobile\", \"odd\"]\n",
        )
        .unwrap();
        for (dir, file) in [("web", "package.json"), ("mobile", "pubspec.yaml")] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
            std::fs::write(root.join(dir).join(file), "").unwrap();
        }
        let project = Project::discover(root).unwrap();

        assert_eq!(
            targets(&project),
            [
                Target::Dotnet(root.join("api")),
                Target::Eslint(root.join("web")),
                Target::Flutter(root.join("mobile")),
                Target::Unknown(root.join("odd")),
            ]
        );
    }
}
