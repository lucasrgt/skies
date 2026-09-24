//! Finds the OpenAPI contract a frontend package generates its client from.
//!
//! The backend build writes its contract to `<backend>/contract/<Project>.json` (the GetDocument tool, configured
//! by `OpenApiDocumentsDirectory`). `Skies.toml` already says which backend a frontend belongs to, so the client
//! generators take no contract argument: the topology is declared once and read here.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::manifest::Project;

/// The backend contract a package's client mirrors, and the stem the client is named after.
#[derive(Debug)]
pub struct Contract {
    pub path: PathBuf,
    /// Lowercase alphanumeric client name derived from the backend project (`Sample.Api` → `sample`).
    pub name: String,
}

/// Resolves the contract for `package` through the nearest `Skies.toml`.
///
/// The product that lists the package as a frontend wins. A single-product workspace also serves a package it
/// does not list, which keeps `skies g client` usable while a new frontend is not yet declared.
pub fn for_package(package: &Path) -> Result<Contract> {
    let project = Project::discover(package)?;
    let backend = backend_for(&project, package)?;
    let dir = if backend.extension().is_some_and(|ext| ext == "csproj") {
        backend.parent().map(Path::to_path_buf).unwrap_or_default()
    } else {
        backend.clone()
    };
    let project_name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let path = contract_file(&dir.join("contract"), &project_name)?;
    Ok(Contract {
        path,
        name: client_name(&project_name),
    })
}

fn backend_for(project: &Project, package: &Path) -> Result<PathBuf> {
    let resolve = |relative: &str| {
        let path = project.root.join(relative);
        path.canonicalize().unwrap_or(path)
    };
    let with_backend: Vec<_> = project
        .manifest
        .products
        .values()
        .filter_map(|p| p.backend.as_deref().map(|b| (p, b)))
        .collect();
    if let Some((_, backend)) = with_backend
        .iter()
        .find(|(product, _)| product.frontend.iter().any(|f| resolve(f) == package))
    {
        return Ok(resolve(backend));
    }
    match with_backend.as_slice() {
        [(_, backend)] => Ok(resolve(backend)),
        [] => bail!(
            "no product in {} declares a backend",
            project.root.join(crate::manifest::FILE_NAME).display()
        ),
        _ => bail!(
            "{} is not a frontend of any product in {}; add it to the product's `frontend` list",
            package.display(),
            project.root.join(crate::manifest::FILE_NAME).display()
        ),
    }
}

fn contract_file(dir: &Path, project_name: &str) -> Result<PathBuf> {
    let missing = || {
        format!(
            "no OpenAPI contract in {}; build the backend first (`dotnet build` emits it)",
            dir.display()
        )
    };
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(dir)
        .with_context(missing)?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    candidates.sort();
    let preferred = dir.join(format!("{project_name}.json"));
    if candidates.contains(&preferred) {
        return Ok(preferred);
    }
    candidates.into_iter().next().with_context(missing)
}

/// `Sample.Api` → `sample`, `Hostpoint.Api` → `hostpoint`: the product part of the project name, reduced to
/// characters that are safe as an orval key, a file name, and part of a Dart package name.
pub fn client_name(project_name: &str) -> String {
    let head = project_name.split('.').next().unwrap_or(project_name);
    let name: String = head
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_lowercase();
    if name.is_empty() {
        "api".to_string()
    } else {
        name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace(toml: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Skies.toml"), toml).unwrap();
        std::fs::create_dir_all(dir.path().join("api/Shop.Api/contract")).unwrap();
        std::fs::write(dir.path().join("api/Shop.Api/contract/Shop.Api.json"), "{}").unwrap();
        std::fs::create_dir_all(dir.path().join("web")).unwrap();
        dir
    }

    #[test]
    fn resolves_the_backend_that_declares_the_package() {
        let dir = workspace(
            "[workspace]\nname = \"s\"\n[products.shop]\nbackend = \"api/Shop.Api\"\nfrontend = \"web\"\n\
             [products.other]\nbackend = \"elsewhere\"\n",
        );
        let package = dir.path().join("web").canonicalize().unwrap();

        let contract = for_package(&package).unwrap();

        assert_eq!(contract.name, "shop");
        assert!(
            contract
                .path
                .ends_with("api/Shop.Api/contract/Shop.Api.json")
        );
    }

    #[test]
    fn explains_a_missing_contract() {
        let dir =
            workspace("[workspace]\nname = \"s\"\n[products.x]\nbackend = \"api/Missing.Api\"\n");
        let package = dir.path().join("web").canonicalize().unwrap();

        let error = format!("{:#}", for_package(&package).unwrap_err());

        assert!(error.contains("build the backend first"), "{error}");
    }

    #[test]
    fn names_the_client_after_the_product() {
        assert_eq!(client_name("Sample.Api"), "sample");
        assert_eq!(client_name("my-shop.Api"), "myshop");
    }
}
