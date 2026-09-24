//! Rewrites a 4.x `Skies.toml` into the 5.x schema: `[workspace]`, `[products.*]` with `backend` and `frontend`,
//! and `[runners.*]`. The 4.x `core`, `library`, and `website` roles collapse into `frontend`, because the only thing
//! Skies 5 does with a frontend package is run its linter; `[framework]` goes away with framework-sync.

use std::fmt::Write as _;
use std::path::Path;

use anyhow::{Context, Result};
use toml::{Table, Value};

use super::Plan;
use crate::manifest::{FILE_NAME, Manifest};

const FRONTEND_ROLES: &[&str] = &["core", "library", "frontend", "website"];

pub fn migrate(root: &Path, plan: &mut Plan) -> Result<()> {
    let path = root.join(FILE_NAME);
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    if toml::from_str::<Manifest>(&text).is_ok() {
        return Ok(());
    }
    let table: Table =
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    let rendered = render(&table, plan);
    plan.write(&path, rendered);
    plan.follow_up(format!(
        "{FILE_NAME} was rewritten (comments are not preserved); declare a [runners.<name>] for each E2E engine"
    ));
    Ok(())
}

fn render(table: &Table, plan: &mut Plan) -> String {
    let name = table
        .get("workspace")
        .and_then(|workspace| workspace.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("app");
    let mut out = String::from(
        "# Skies workspace manifest: the product topology `skies doctor` checks and the runners `skies proof` uses.\n",
    );
    let _ = write!(out, "\n[workspace]\nname = {}\n", quote(name));

    if let Some(products) = table.get("products").and_then(Value::as_table) {
        for (product, entry) in products {
            let Some(entry) = entry.as_table() else {
                continue;
            };
            let _ = write!(out, "\n[products.{product}]\n");
            if let Some(backend) = entry.get("backend").and_then(Value::as_str) {
                let _ = writeln!(out, "backend = {}", quote(backend));
            }
            let frontends = frontend_paths(entry);
            match frontends.as_slice() {
                [] => {}
                [one] => {
                    let _ = writeln!(out, "frontend = {}", quote(one));
                }
                many => {
                    let list: Vec<String> = many.iter().map(|path| quote(path)).collect();
                    let _ = writeln!(out, "frontend = [{}]", list.join(", "));
                }
            }
        }
    }

    match table.get("runners").and_then(Value::as_table) {
        Some(runners) => {
            let mut section = Table::new();
            section.insert("runners".into(), Value::Table(runners.clone()));
            let _ = write!(out, "\n{}", toml::to_string(&section).unwrap_or_default());
        }
        None => out.push_str(
            "\n# A runner runs one spec's e2e/ folder and writes a JUnit or TRX report. Placeholders: {id}, {spec},\n\
             # {dir}, {report}, {evidence}.\n\
             # [runners.api]\n\
             # command = \"dotnet test tests/App.Tests --filter FullyQualifiedName~Specs.S{id}. --logger trx;LogFilePath={report}\"\n",
        ),
    }

    let dropped: Vec<&str> = table
        .keys()
        .map(String::as_str)
        .filter(|key| !["workspace", "products", "runners"].contains(key))
        .collect();
    if !dropped.is_empty() {
        plan.follow_up(format!(
            "{FILE_NAME}: dropped section(s) {}",
            dropped.join(", ")
        ));
    }
    out
}

/// The product's frontend packages in declaration order of their 4.x roles, without duplicates.
fn frontend_paths(entry: &Table) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    for role in FRONTEND_ROLES {
        let values: Vec<&str> = match entry.get(*role) {
            Some(Value::String(one)) => vec![one.as_str()],
            Some(Value::Array(many)) => many.iter().filter_map(Value::as_str).collect(),
            _ => Vec::new(),
        };
        for value in values {
            if !paths.iter().any(|known| known == value) {
                paths.push(value.to_string());
            }
        }
    }
    paths
}

fn quote(value: &str) -> String {
    Value::String(value.to_string()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapses_frontend_roles_and_drops_framework() {
        let table: Table = toml::from_str(
            "[workspace]\nname = \"hp\"\n[products.app]\nbackend = \"src/Hp.Api\"\ncore = \"clients/core\"\nfrontend = \"clients/app\"\nwebsite = \"clients/site\"\n[products.ui]\nlibrary = \"clients/ui\"\n[framework]\nrepo = \"../skies\"\n",
        )
        .unwrap();
        let mut plan = Plan::default();

        let text = render(&table, &mut plan);
        let manifest: Manifest = toml::from_str(&text).unwrap();

        assert_eq!(
            manifest.products["app"].frontend.iter().collect::<Vec<_>>(),
            ["clients/core", "clients/app", "clients/site"]
        );
        assert_eq!(
            manifest.products["ui"].frontend.iter().collect::<Vec<_>>(),
            ["clients/ui"]
        );
        assert!(
            plan.follow_ups
                .keys()
                .any(|note| note.contains("framework"))
        );
    }
}
