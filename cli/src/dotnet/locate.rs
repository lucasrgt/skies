//! Which API project a backend generator writes into.
//!
//! In order: `--project` when given; the current directory when it holds a `.csproj` (the 4.x habit); the backend
//! of the nearest `Skies.toml` whose folder contains the current directory; the only backend `Skies.toml` declares;
//! and, when it declares several, the one whose `Modules/<Module>` already exists. Anything else is a user error
//! that names the candidates and the flag that settles it, so `skies g slice` works from the app root like every
//! other command.

use std::path::{Path, PathBuf};

use crate::manifest::{FILE_NAME, Project};

/// The API project directory, or the message that explains why none could be chosen.
pub fn project_dir(cwd: &Path, explicit: Option<&Path>, module: Option<&str>) -> Result<PathBuf, String> {
    if let Some(path) = explicit {
        let path = cwd.join(path);
        let dir = if path.extension().is_some_and(|ext| ext == "csproj") {
            path.parent().map(Path::to_path_buf).unwrap_or_default()
        } else {
            path
        };
        return if super::first_csproj(&dir).is_some() {
            Ok(dir)
        } else {
            Err(format!("--project {}: no .csproj there.", dir.display()))
        };
    }
    if super::first_csproj(cwd).is_some() {
        return Ok(cwd.to_path_buf());
    }
    let Ok(project) = Project::discover(cwd) else {
        return Err(format!(
            "no .csproj here and no {FILE_NAME} above it — run this from the application project directory, or \
             pass --project <dir>."
        ));
    };
    let backends = backends(&project);
    if let Some(enclosing) = backends.iter().find(|dir| cwd.starts_with(dir)) {
        return Ok(enclosing.clone());
    }
    match backends.as_slice() {
        [] => Err(format!(
            "no .csproj here and {} declares no backend with one — run this from the application project \
             directory, or pass --project <dir>.",
            project.root.join(FILE_NAME).display()
        )),
        [only] => Ok(only.clone()),
        several => by_module(several, module).ok_or_else(|| {
            let names: Vec<String> = several
                .iter()
                .map(|dir| dir.strip_prefix(&project.root).unwrap_or(dir).display().to_string())
                .collect();
            let reason = match module {
                Some(module) => format!("Modules/{module} is not in exactly one of them"),
                None => "this generator is not tied to a module".to_string(),
            };
            format!(
                "{FILE_NAME} declares several backends ({}) and {reason}; pass --project <dir> or run this from \
                 the project directory.",
                names.join(", ")
            )
        }),
    }
}

/// Every declared backend that holds a project, as a directory, in manifest order and without repeats.
fn backends(project: &Project) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for product in project.manifest.products.values() {
        let Some(backend) = product.backend.as_deref() else {
            continue;
        };
        let path = project.root.join(backend);
        let dir = if path.extension().is_some_and(|ext| ext == "csproj") {
            path.parent().map(Path::to_path_buf).unwrap_or_default()
        } else {
            path
        };
        if super::first_csproj(&dir).is_some() && !out.contains(&dir) {
            out.push(dir);
        }
    }
    out
}

fn by_module(backends: &[PathBuf], module: Option<&str>) -> Option<PathBuf> {
    let module = module?;
    let mut owners = backends.iter().filter(|dir| dir.join("Modules").join(module).is_dir());
    match (owners.next(), owners.next()) {
        (Some(owner), None) => Some(owner.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(toml_products: &str, projects: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(FILE_NAME),
            format!("[workspace]\nname = \"a\"\n{toml_products}"),
        )
        .unwrap();
        for project in projects {
            let path = dir.path().join(project);
            std::fs::create_dir_all(&path).unwrap();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            std::fs::write(path.join(format!("{name}.csproj")), "<Project />").unwrap();
        }
        dir
    }

    #[test]
    fn the_app_root_resolves_the_declared_backend() {
        let dir = app("[products.app]\nbackend = \"src/Acme.Api\"\n", &["src/Acme.Api"]);
        assert_eq!(
            project_dir(dir.path(), None, Some("Billing")).unwrap(),
            dir.path().join("src/Acme.Api")
        );
        let inside = dir.path().join("src/Acme.Api/Modules");
        std::fs::create_dir_all(&inside).unwrap();
        assert_eq!(
            project_dir(&inside, None, None).unwrap(),
            dir.path().join("src/Acme.Api")
        );
    }

    #[test]
    fn a_project_directory_and_an_explicit_project_win() {
        let dir = app(
            "[products.app]\nbackend = \"src/Acme.Api\"\n",
            &["src/Acme.Api", "src/Other.Api"],
        );
        let other = dir.path().join("src/Other.Api");
        assert_eq!(project_dir(&other, None, None).unwrap(), other);
        assert_eq!(
            project_dir(dir.path(), Some(Path::new("src/Other.Api/Other.Api.csproj")), None).unwrap(),
            other
        );
        assert!(
            project_dir(dir.path(), Some(Path::new("src")), None)
                .unwrap_err()
                .contains("no .csproj")
        );
    }

    #[test]
    fn several_backends_are_told_apart_by_module_or_refused() {
        let dir = app(
            "[products.a]\nbackend = \"a/A.Api\"\n[products.b]\nbackend = \"b/B.Api/B.Api.csproj\"\n",
            &["a/A.Api", "b/B.Api"],
        );
        std::fs::create_dir_all(dir.path().join("b/B.Api/Modules/Billing")).unwrap();

        assert_eq!(
            project_dir(dir.path(), None, Some("Billing")).unwrap(),
            dir.path().join("b/B.Api")
        );
        let refused = project_dir(dir.path(), None, Some("Catalog")).unwrap_err();
        assert!(
            refused.contains("a/A.Api, b/B.Api") && refused.contains("--project"),
            "{refused}"
        );
        assert!(
            project_dir(dir.path(), None, None)
                .unwrap_err()
                .contains("not tied to a module")
        );
    }

    #[test]
    fn outside_any_app_the_message_says_what_to_do() {
        let dir = tempfile::tempdir().unwrap();
        let error = project_dir(dir.path(), None, None).unwrap_err();
        assert!(
            error.contains("no .csproj here") && error.contains("--project"),
            "{error}"
        );
    }
}
