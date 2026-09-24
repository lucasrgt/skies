//! `skies new <Name>`: render the embedded solution template into `./<Name>`.
//!
//! The template is still a valid `dotnet new` template (`.template.config/template.json`), and this renderer
//! keeps its one substitution rule: the `sourceName` is replaced by the app name in every path and every text
//! file. Rendering it here instead of shelling out to `dotnet new install` means no template package to install,
//! no machine-wide template registry to pollute, and the same result offline.

use std::path::Path;

use anyhow::{Context, Result};

use super::{embedded, text};

const CONFIG_DIR: &str = ".template.config/";

pub fn new_app(cwd: &Path, name: &str) -> Result<u8> {
    if !is_dotted_identifier(name) {
        eprintln!(
            "skies: '{name}' is not a valid application name — use a C# namespace such as Acme or Acme.Billing \
             (letters, digits, and underscores; dots between parts)."
        );
        return Ok(1);
    }
    let target = cwd.join(name);
    if target.exists() && std::fs::read_dir(&target)?.next().is_some() {
        eprintln!("skies: {} already exists and is not empty.", target.display());
        return Ok(1);
    }

    let files = embedded::app_files();
    let source_name = source_name(&files)?;
    let mut written = 0;
    for (path, contents) in &files {
        if path.starts_with(CONFIG_DIR) {
            continue;
        }
        let destination = target.join(path.replace(&source_name, name));
        match std::str::from_utf8(contents) {
            Ok(body) => text::write(&destination, body.replace(&source_name, name))?,
            Err(_) => text::write(&destination, contents)?,
        }
        written += 1;
    }

    println!("created {} ({written} files)", target.display());
    println!(
        "next: `cd {name} && dotnet build`, then describe your first feature with `skies spec new <slug>` \
         and scaffold it with `skies g module|slice|entity` from src/{name}.Api."
    );
    Ok(0)
}

/// The template's `sourceName`: the placeholder `dotnet new` replaces, read from the template's own config so
/// the two renderers cannot disagree.
fn source_name(files: &[(String, &[u8])]) -> Result<String> {
    let (_, config) = files
        .iter()
        .find(|(path, _)| path == ".template.config/template.json")
        .context("the embedded app template has no .template.config/template.json")?;
    let json: serde_json::Value = serde_json::from_slice(config).context("parsing template.json")?;
    json["sourceName"]
        .as_str()
        .map(str::to_string)
        .context("template.json has no sourceName")
}

/// `Acme` or `Acme.Billing`: the name becomes the solution file, project names, and root namespace, so it has
/// to be a namespace. `dotnet new` would silently rewrite `my-app` to `my_app` in some places and not others.
fn is_dotted_identifier(name: &str) -> bool {
    !name.is_empty()
        && name.split('.').all(|part| {
            let mut chars = part.chars();
            chars.next().is_some_and(|c| c.is_alphabetic() || c == '_')
                && chars.all(|c| c.is_alphanumeric() || c == '_')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_the_solution_under_the_app_name() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(new_app(dir.path(), "Acme").unwrap(), 0);

        let root = dir.path().join("Acme");
        assert!(root.join("Acme.slnx").exists());
        assert!(root.join(".specs/README.md").exists());
        assert!(!root.join(".template.config").exists());
        let csproj = std::fs::read_to_string(root.join("tests/Acme.Tests/Acme.Tests.csproj")).unwrap();
        assert!(csproj.contains(r#"<Compile Include="..\..\.specs\*\e2e\**\*.cs" />"#));
        assert!(csproj.contains(r"..\..\src\Acme.Api\Acme.Api.csproj"));
        let manifest = crate::manifest::load(&root.join("Skies.toml")).unwrap();
        assert_eq!(manifest.workspace.name, "Acme");
        assert_eq!(manifest.products["app"].backend.as_deref(), Some("src/Acme.Api"));
        // Every test lives in a spec: the tests project compiles nothing beside the spec cases, runs the doctor, and
        // is what `skies doctor` builds.
        assert!(!csproj.contains("*.Tests.cs"));
        assert!(csproj.contains(r#"<PackageReference Include="Skies.Framework.Doctor""#));
        assert_eq!(manifest.products["app"].tests.as_deref(), Some("tests/Acme.Tests"));

        assert_eq!(
            new_app(dir.path(), "Acme").unwrap(),
            1,
            "never renders over an existing app"
        );
    }

    #[test]
    fn a_new_app_declares_the_api_runner_and_ignores_build_output() {
        let dir = tempfile::tempdir().unwrap();
        new_app(dir.path(), "Acme").unwrap();
        let manifest = std::fs::read_to_string(dir.path().join("Acme/Skies.toml")).unwrap();
        let parsed: crate::manifest::Manifest = toml::from_str(&manifest).unwrap();
        assert!(parsed.runners["api"].command.contains("Specs.S{id}."));
        let ignored = std::fs::read_to_string(dir.path().join("Acme/.gitignore")).unwrap();
        assert!(ignored.lines().any(|line| line == "bin/") && ignored.lines().any(|line| line == "obj/"));
    }

    #[test]
    fn rejects_names_that_are_not_namespaces() {
        let dir = tempfile::tempdir().unwrap();
        for bad in ["", "my-app", "1app", "a..b", "a/b"] {
            assert_eq!(new_app(dir.path(), bad).unwrap(), 1, "{bad}");
        }
    }
}
