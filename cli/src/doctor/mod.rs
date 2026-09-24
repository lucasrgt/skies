//! `skies doctor`: runs each platform's architecture analyzers and reports them together.
//!
//! Every platform already owns its checks: the SKY Roslyn analyzers run inside `dotnet build`, the SKYFE rules
//! inside the package's `npm run lint`, and the SKYFL rules natively here. A React package also gets a typecheck leg
//! (its `typecheck` script, else `tsc --noEmit -p`), because a type error is invisible to ESLint and a View bound to a
//! renamed hook field lints clean. The workspace leg checks the repository root against `[workspace] root`
//! (SKYWS001/SKYWS002, see [`workspace`]): the layout is architecture too. The doctor only finds the packages
//! `Skies.toml` declares, runs their legs in parallel, and prints one table. It is run on purpose; no hook calls it,
//! and it polices nothing but architecture.
//!
//! `--package <dir>` runs only the leg for that one directory, so a package's own `lint` script can call the doctor
//! without a `Skies.toml` lookup and without linting its siblings (or the root).
//!
//! Exit codes: 0 when no leg has an error-level finding, 1 when one does, 2 when a leg could not run at all
//! (a missing tool, a package without a manifest). Warnings are reported but never fail the run.

mod legs;
mod report;
pub mod workspace;

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
    /// The repository root, checked against `Skies.toml` `[workspace] root` (`None` when the key is missing).
    Workspace(PathBuf, Option<Vec<String>>),
    Dotnet(PathBuf),
    Eslint(PathBuf),
    /// A React package's TypeScript check, run beside its lint.
    Typecheck(PathBuf),
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

pub fn run(build_args: &[String], package: Option<&Path>) -> Result<u8> {
    if let Some(package) = package {
        return run_package(package, build_args);
    }
    let project = Project::from_cwd()?;
    let packages = targets(&project);
    if packages.is_empty() {
        println!(
            "skies doctor: {} ({}) declares no backend or frontend packages; checking the root only",
            project.manifest.workspace.name,
            crate::manifest::FILE_NAME
        );
    } else {
        println!("skies doctor: {}", project.manifest.workspace.name);
    }
    let mut targets = vec![Target::Workspace(
        project.root.clone(),
        project.manifest.workspace.root.clone(),
    )];
    targets.extend(packages);
    let started = Instant::now();
    let legs = run_legs(&project.root, &targets, build_args);
    print!("{}", report::render(&project.root, &legs, started.elapsed()));
    Ok(exit_code(&legs))
}

/// Runs the one leg `package` calls for. Findings print relative to the workspace root when there is one, so they
/// read the same as in a full run.
fn run_package(package: &Path, build_args: &[String]) -> Result<u8> {
    let dir = package
        .canonicalize()
        .map_err(|_| anyhow::anyhow!("package directory {} does not exist", package.display()))?;
    let target = classify_package(&dir);
    if let Target::Unknown(_) = target {
        anyhow::bail!(
            "{} has no pubspec.yaml, package.json, or .NET project to check",
            dir.display()
        );
    }
    // Outside a workspace, paths print relative to the package's parent so the leg still names the package.
    let root = Project::discover(&dir).map_or_else(
        |_| dir.parent().map_or_else(|| dir.clone(), Path::to_path_buf),
        |project| project.root,
    );
    println!("skies doctor: {}", dir.strip_prefix(&root).unwrap_or(&dir).display());
    let started = Instant::now();
    let legs = run_legs(&root, &with_typecheck(vec![target]), build_args);
    print!("{}", report::render(&root, &legs, started.elapsed()));
    Ok(exit_code(&legs))
}

/// A frontend manifest wins over .NET files, so a Flutter package with a stray `.csproj` fixture stays a Flutter
/// package; a project or solution file (or a directory holding one) is a backend.
fn classify_package(path: &Path) -> Target {
    let is_dotnet = |file: &Path| {
        file.extension()
            .is_some_and(|ext| ext == "csproj" || ext == "sln" || ext == "slnx")
    };
    if path.is_file() {
        return if is_dotnet(path) {
            Target::Dotnet(path.to_path_buf())
        } else {
            Target::Unknown(path.to_path_buf())
        };
    }
    match classify(path) {
        Target::Unknown(dir) => {
            let holds_project = std::fs::read_dir(&dir)
                .map(|entries| entries.flatten().any(|entry| is_dotnet(&entry.path())))
                .unwrap_or(false);
            if holds_project {
                Target::Dotnet(dir)
            } else {
                Target::Unknown(dir)
            }
        }
        known => known,
    }
}

/// One target per declared package, in manifest order (products are sorted by name). A declared tests project
/// replaces the backend as the .NET target: building it builds the backend too, so the analyzers see both, and two
/// parallel builds of the same backend would fight over its output.
pub fn targets(project: &Project) -> Vec<Target> {
    let mut out = Vec::new();
    for product in project.manifest.products.values() {
        if let Some(dotnet) = product.tests.as_ref().or(product.backend.as_ref()) {
            out.push(Target::Dotnet(project.root.join(dotnet)));
        }
        for frontend in product.frontend.iter() {
            out.push(classify(&project.root.join(frontend)));
        }
    }
    with_typecheck(out)
}

/// Adds a typecheck leg right after every React package's lint leg.
fn with_typecheck(targets: Vec<Target>) -> Vec<Target> {
    let mut out = Vec::with_capacity(targets.len());
    for target in targets {
        let typecheck = match &target {
            Target::Eslint(dir) => Some(Target::Typecheck(dir.clone())),
            _ => None,
        };
        out.push(target);
        out.extend(typecheck);
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
    fn a_package_directory_selects_its_own_leg() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for (dir, file) in [
            ("web", "package.json"),
            ("mobile", "pubspec.yaml"),
            ("api", "Api.csproj"),
            ("odd", "notes.txt"),
        ] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
            std::fs::write(root.join(dir).join(file), "").unwrap();
        }
        std::fs::write(root.join("mobile/Fixture.csproj"), "").unwrap();

        assert_eq!(classify_package(&root.join("web")), Target::Eslint(root.join("web")));
        assert_eq!(
            classify_package(&root.join("mobile")),
            Target::Flutter(root.join("mobile"))
        );
        assert_eq!(classify_package(&root.join("api")), Target::Dotnet(root.join("api")));
        assert_eq!(
            classify_package(&root.join("api/Api.csproj")),
            Target::Dotnet(root.join("api/Api.csproj"))
        );
        assert_eq!(classify_package(&root.join("odd")), Target::Unknown(root.join("odd")));
    }

    #[test]
    fn targets_follow_the_manifest_and_classify_frontends_by_manifest_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("Skies.toml"),
            "[workspace]\nname = \"d\"\n[products.app]\nbackend = \"api\"\nfrontend = [\"web\", \"mobile\", \"odd\"]\n\
             [products.tested]\nbackend = \"b/api\"\ntests = \"b/tests\"\n",
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
                Target::Typecheck(root.join("web")),
                Target::Flutter(root.join("mobile")),
                Target::Unknown(root.join("odd")),
                Target::Dotnet(root.join("b/tests")),
            ]
        );
    }
}
