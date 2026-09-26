//! `skies g web-app`: a minimal, runnable React web package on the stack the frontend conventions assume.
//!
//! Vite + React + TypeScript strict, TanStack Router (code-based, typed routes), TanStack Query, react-hook-form +
//! zod, react-i18next, the `@skiesjs/react` spine, and `@skiesjs/eslint-plugin`'s recommended config. The package
//! carries what a feature scaffold imports (`@/ui`, `@/i18n`, `@/client.gen/<api>` through the `@/` alias) and the
//! hand-owned client seams `skies g client` would write, so it builds, typechecks, and lints before a backend
//! exists. Inside a Skies application it is declared in `Skies.toml` (a product's `frontend`, the root allowlist) so
//! `skies doctor` checks it and `skies g client` finds its backend.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use include_dir::{Dir, include_dir};
use minijinja::context;

use super::names::kebab;
use super::scaffold::{relative_path, render, write_new};
use super::{ci, client, i18n, register};
use crate::manifest::Project;

static APP: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/templates/react/app");

/// A template file rendered through minijinja carries this suffix; every other file is copied as is (JSX's
/// `style={{ ... }}` would read as a placeholder).
const RENDERED: &str = ".jinja";

pub fn create(name: &str, path: Option<&Path>) -> Result<u8> {
    let package = kebab(name);
    if !package.starts_with(|c: char| c.is_ascii_lowercase()) {
        bail!("'{name}' does not make an npm package name; start it with a letter");
    }
    let cwd = std::env::current_dir()?;
    let dir = cwd.join(path.unwrap_or(Path::new(name)));
    if dir.exists() && std::fs::read_dir(&dir)?.next().is_some() {
        bail!("{} already exists and is not empty", dir.display());
    }
    let parent = dir.parent().unwrap_or(Path::new("/"));
    std::fs::create_dir_all(parent)?;
    let dir = parent.canonicalize()?.join(dir.file_name().unwrap_or(name.as_ref()));
    let project = Project::discover(&dir).ok();
    let app_root = project.as_ref().map_or_else(|| dir.clone(), |p| p.root.clone());

    write_new(&package_files(&dir, &package, name, &app_root)?)?;
    i18n::assemble(&dir)?;

    let relative = project.as_ref().map(|p| relative_path(&p.root, &dir));
    let package_path = relative
        .as_deref()
        .map_or_else(|| dir.display().to_string(), |r| r.trim_start_matches("./").to_string());
    let (setup, command) = web_runner(&package_path);
    if let Some(project) = &project {
        register::frontend(&project.root, &package_path)?;
        register::runner(&project.root, "web", &setup, &command)?;
        ci::add_package(&project.root, &package_path)?;
    }
    println!("\ncreated React web app {package} in {}", dir.display());
    println!(
        "next: `npm install --prefix {package_path}`, then `npm run dev` there. Once the backend has built its \
         contract: `skies g client --package {package_path}` and `skies g feature <Name> --package {package_path}`."
    );
    if project.is_none() {
        println!(
            "\nTo run web specs (.specs/<id>/e2e/*.test.tsx) with `skies proof`, declare a runner in Skies.toml:\n\n\
             [runners.web]\nsetup = {}\ncommand = {}",
            toml::Value::String(setup),
            toml::Value::String(command)
        );
    }
    Ok(0)
}

/// The `[runners.web]` that runs one spec's web cases with the package's own vitest (its config roots the run at the
/// application, where `.specs/` lives): `setup` installs the package once per checkout, `command` writes JUnit.
fn web_runner(package: &str) -> (String, String) {
    (
        format!("test -d {package}/node_modules || npm --prefix {package} ci"),
        format!(
            "node {package}/node_modules/vitest/vitest.mjs run --config {package}/vitest.config.ts \
             --reporter=junit --outputFile={{report}} {{dir}}"
        ),
    )
}

/// Every file of the package, rendered for `package` (the npm name) under `dir`. `app_root` is where `.specs/`
/// lives: the application root, or the package itself outside a Skies application.
pub fn package_files(dir: &Path, package: &str, title: &str, app_root: &Path) -> Result<Vec<(PathBuf, String)>> {
    let root = if app_root == dir {
        "./".to_string()
    } else {
        format!("{}/", relative_path(dir, app_root))
    };
    let ctx = context! {
        package,
        title,
        version => crate::dotnet::FRAMEWORK_VERSION,
        app_root => root,
    };
    let mut files = Vec::new();
    for file in template_files(&APP) {
        let relative = file.path().to_string_lossy().replace('\\', "/");
        let text = file.contents_utf8().unwrap_or_default();
        match relative.strip_suffix(RENDERED) {
            Some(target) => files.push((dir.join(target), render(text, &ctx)?)),
            None => files.push((dir.join(&relative), text.to_string())),
        }
    }
    for (file, contents) in client::render_boot_seams() {
        files.push((dir.join(file), contents));
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(files)
}

fn template_files(dir: &'static Dir<'static>) -> Vec<&'static include_dir::File<'static>> {
    let mut out: Vec<_> = dir.files().collect();
    for child in dir.dirs() {
        out.extend(template_files(child));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered(app_root: &str) -> Vec<(String, String)> {
        let dir = Path::new("/app/clients/web");
        package_files(dir, "web", "Web", Path::new(app_root))
            .unwrap()
            .into_iter()
            .map(|(path, text)| (path.strip_prefix(dir).unwrap().display().to_string(), text))
            .collect()
    }

    fn file<'a>(files: &'a [(String, String)], name: &str) -> &'a str {
        &files
            .iter()
            .find(|(path, _)| path == name)
            .unwrap_or_else(|| panic!("{name}"))
            .1
    }

    #[test]
    fn the_package_carries_what_a_feature_imports_and_the_boot_seams() {
        let files = rendered("/app");
        let names: Vec<&str> = files.iter().map(|(path, _)| path.as_str()).collect();
        for expected in [
            "package.json",
            "index.html",
            "vite.config.ts",
            "vitest.config.ts",
            "tsconfig.json",
            "eslint.config.js",
            ".gitignore",
            ".env.example",
            "src/main.tsx",
            "src/i18n.ts",
            "src/routes/router.ts",
            "src/ui/index.ts",
            "src/lib/skies-client.ts",
            "src/lib/query.ts",
            "src/lib/feedback.ts",
        ] {
            assert!(names.contains(&expected), "{expected} missing from {names:?}");
        }
        assert!(
            !names
                .iter()
                .any(|n| n.ends_with(RENDERED) || n.contains("orval.config"))
        );
        for (path, text) in &files {
            for placeholder in ["{{ package", "{{ title", "{{ version", "{{ app_root", "{%"] {
                assert!(!text.contains(placeholder), "{path} kept {placeholder}");
            }
        }
        assert!(file(&files, "src/shell/shell.i18n.ts").contains("title: \"Web\","));
    }

    #[test]
    fn the_manifest_pins_the_lockstep_spine_and_the_three_scripts() {
        let files = rendered("/app");
        let manifest: serde_json::Value = serde_json::from_str(file(&files, "package.json")).unwrap();
        let version = crate::dotnet::FRAMEWORK_VERSION;
        assert_eq!(manifest["name"], "web");
        assert_eq!(manifest["dependencies"]["@skiesjs/react"], version);
        assert_eq!(manifest["devDependencies"]["@skiesjs/eslint-plugin"], version);
        for script in ["lint", "typecheck", "test", "build"] {
            assert!(manifest["scripts"][script].is_string(), "{script}");
        }
        assert!(manifest["dependencies"]["@tanstack/react-router"].is_string());
        assert!(file(&files, "tsconfig.json").contains("\"@/*\": [\"./src/*\"]"));
        assert!(file(&files, "eslint.config.js").contains("skies.configs.recommended"));
    }

    #[test]
    fn specs_run_from_the_application_root() {
        let inside = rendered("/app");
        assert!(file(&inside, "vitest.config.ts").contains("new URL(\"../../\", import.meta.url)"));
        let alone = rendered("/app/clients/web");
        assert!(file(&alone, "vitest.config.ts").contains("new URL(\"./\", import.meta.url)"));
    }

    #[test]
    fn the_kit_speaks_dom_props() {
        let files = rendered("/app");
        let button = file(&files, "src/ui/Button.tsx");
        let input = file(&files, "src/ui/Input.tsx");
        assert!(button.contains("onClick: () => void;") && input.contains("onChange: (event: ChangeEvent"));
        for (path, text) in &files {
            assert!(!text.contains("onPress") && !text.contains("onChangeText"), "{path}");
        }
    }

    #[test]
    fn refuses_a_name_without_letters_and_a_non_empty_folder() {
        assert!(create("--", None).is_err());
    }
}
