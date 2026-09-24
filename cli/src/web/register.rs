//! Declares a new frontend package in `Skies.toml`, so the scaffolded package is a checked part of the application
//! from its first commit: `skies doctor` lints and typechecks every product's `frontend`, `skies g client` finds the
//! package's backend through it, and the root allowlist (`[workspace] root`) has to name the folder it sits in.
//!
//! The manifest is edited as text, at known anchors, keeping every other byte (comments included); each edit is read
//! back through the parser and dropped if it does not mean what it should. When an anchor is missing or the choice is
//! ambiguous (several products), the step to take by hand is printed instead.

use std::path::Path;

use anyhow::{Context, Result};

use crate::doctor::workspace::{Allowlist, Entry};
use crate::manifest::{FILE_NAME, Manifest};

/// Declares `package` (a path relative to `root`, forward slashes) as a frontend and its top folder as a root entry.
pub fn frontend(root: &Path, package: &str) -> Result<()> {
    let path = root.join(FILE_NAME);
    let mut text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let manifest: Manifest = toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    let mut notes = Vec::new();

    let listed = manifest
        .products
        .values()
        .any(|product| product.frontend.iter().any(|f| f.trim_end_matches('/') == package));
    let products: Vec<&String> = manifest.products.keys().collect();
    if !listed {
        match products.as_slice() {
            [product] => match with_frontend(&text, product, package) {
                Some(updated) => {
                    text = updated;
                    println!("declared {package} as a frontend of [products.{product}] in {FILE_NAME}");
                }
                None => notes.push(format!("add \"{package}\" to [products.{product}] frontend")),
            },
            _ => notes.push(format!(
                "add \"{package}\" to the `frontend` list of the product it serves"
            )),
        }
    }

    if let Some(declared) = &manifest.workspace.root {
        let top = package.split('/').next().unwrap_or(package).to_string();
        let entry = Entry {
            is_dir: true,
            name: top,
        };
        let allowed = Allowlist::compile(declared).is_ok_and(|list| list.allows(&entry));
        if !allowed {
            match with_root_entry(&text, &entry.pattern()) {
                Some(updated) => {
                    text = updated;
                    println!("declared {} in [workspace] root", entry.display());
                }
                None => notes.push(format!("add \"{}\" to [workspace] root", entry.pattern())),
            }
        }
    }

    std::fs::write(&path, &text).with_context(|| format!("writing {}", path.display()))?;
    for note in notes {
        println!("note: {FILE_NAME}: {note}");
    }
    Ok(())
}

/// The manifest with `package` added to `product`'s `frontend`: a one-line `frontend` gains the path (a string
/// becomes a list), a commented `# frontend = ...` placeholder is replaced, else the key is added after the table's
/// last key. `None` when the result would not read back with the package listed.
pub fn with_frontend(text: &str, product: &str, package: &str) -> Option<String> {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let header = lines
        .iter()
        .position(|line| is_header(line, &format!("products.{product}")))?;
    let end = lines[header + 1..]
        .iter()
        .position(|line| line.trim_start().starts_with('['))
        .map_or(lines.len(), |offset| header + 1 + offset);
    let quoted = toml::Value::String(package.to_string()).to_string();
    let key = |line: &str, commented: bool| {
        let line = line.trim_start();
        let line = if commented {
            line.strip_prefix('#').map(str::trim_start)
        } else {
            Some(line)
        };
        line.is_some_and(|l| l.starts_with("frontend") && l["frontend".len()..].trim_start().starts_with('='))
    };
    if let Some(at) = (header + 1..end).find(|&i| key(&lines[i], false)) {
        let manifest: Manifest = toml::from_str(text).ok()?;
        let mut current: Vec<String> = manifest
            .products
            .get(product)?
            .frontend
            .iter()
            .map(str::to_string)
            .collect();
        if lines[at].contains('[') && !lines[at].contains(']') {
            return None;
        }
        current.push(package.to_string());
        let list: Vec<String> = current
            .iter()
            .map(|p| toml::Value::String(p.clone()).to_string())
            .collect();
        lines[at] = format!("frontend = [{}]", list.join(", "));
    } else if let Some(at) = (header + 1..end).find(|&i| key(&lines[i], true)) {
        lines[at] = format!("frontend = {quoted}");
    } else {
        let last_key = (header + 1..end)
            .rev()
            .find(|&i| {
                let line = lines[i].trim_start();
                !line.is_empty() && !line.starts_with('#')
            })
            .unwrap_or(header);
        lines.insert(last_key + 1, format!("frontend = {quoted}"));
    }
    let out = lines.join("\n") + "\n";
    let reread: Manifest = toml::from_str(&out).ok()?;
    reread
        .products
        .get(product)?
        .frontend
        .iter()
        .any(|f| f == package)
        .then_some(out)
}

/// The manifest with `pattern` added to `[workspace] root`, after its last folder entry in a multi-line list, or at
/// the end of a one-line list. `None` when the result would not read back with the pattern declared.
pub fn with_root_entry(text: &str, pattern: &str) -> Option<String> {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let header = lines.iter().position(|line| is_header(line, "workspace"))?;
    let end = lines[header + 1..]
        .iter()
        .position(|line| line.trim_start().starts_with('['))
        .map_or(lines.len(), |offset| header + 1 + offset);
    let start = (header + 1..end).find(|&i| {
        let line = lines[i].trim_start();
        line.starts_with("root") && line["root".len()..].trim_start().starts_with('=')
    })?;
    let quoted = toml::Value::String(pattern.to_string()).to_string();
    if lines[start].contains(']') {
        let close = lines[start].rfind(']')?;
        let before = lines[start][..close].trim_end();
        let separator = if before.ends_with('[') || before.ends_with(',') {
            ""
        } else {
            ", "
        };
        lines[start] = format!("{before}{separator}{quoted}{}", &lines[start][close..]);
    } else {
        let close = (start + 1..lines.len()).find(|&i| lines[i].trim_start().starts_with(']'))?;
        let last_folder = (start + 1..close).rev().find(|&i| lines[i].trim().ends_with("/\","));
        let at = last_folder.map_or(close, |i| i + 1);
        lines.insert(at, format!("  {quoted},"));
    }
    let out = lines.join("\n") + "\n";
    let reread: Manifest = toml::from_str(&out).ok()?;
    reread.workspace.root?.iter().any(|p| p == pattern).then_some(out)
}

fn is_header(line: &str, name: &str) -> bool {
    let line = line.trim();
    line.strip_prefix('[')
        .and_then(|rest| rest.split(']').next())
        .is_some_and(|inner| !line.starts_with("[[") && inner.trim() == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEMPLATE: &str = "[workspace]\nname = \"Acme\"\nroot = [\n  \".specs/\",\n  \"src/\",\n  \"README.md\",\n]\n\n\
         [products.app]\nbackend = \"src/Acme.Api\"   # the API\ntests = \"tests/Acme.Tests\"\n\
         # frontend = \"clients/web\"   # declared when you add one\n\n[runners.api]\ncommand = \"dotnet test\"\n";

    #[test]
    fn replaces_the_commented_placeholder_and_keeps_every_comment() {
        let out = with_frontend(TEMPLATE, "app", "clients/web").unwrap();
        assert!(out.contains("tests = \"tests/Acme.Tests\"\nfrontend = \"clients/web\"\n\n[runners.api]"));
        assert!(out.contains("backend = \"src/Acme.Api\"   # the API"));
    }

    #[test]
    fn a_second_frontend_turns_the_key_into_a_list() {
        let once = with_frontend(TEMPLATE, "app", "clients/web").unwrap();
        let twice = with_frontend(&once, "app", "clients/admin").unwrap();
        assert!(twice.contains("frontend = [\"clients/web\", \"clients/admin\"]\n"));
    }

    #[test]
    fn a_product_without_the_placeholder_gets_the_key_after_its_last_one() {
        let text = "[workspace]\nname = \"x\"\n[products.shop]\nbackend = \"api\"\n\n# runners\n[runners.api]\ncommand = \"c\"\n";
        let out = with_frontend(text, "shop", "web").unwrap();
        assert!(out.contains("backend = \"api\"\nfrontend = \"web\"\n\n# runners"));
    }

    #[test]
    fn a_folder_joins_the_root_list_after_the_last_folder() {
        let out = with_root_entry(TEMPLATE, "clients/").unwrap();
        assert!(out.contains("  \"src/\",\n  \"clients/\",\n  \"README.md\",\n]"));
        let inline = with_root_entry("[workspace]\nname = \"x\"\nroot = [\"src/\"]\n", "web/").unwrap();
        assert!(inline.contains("root = [\"src/\", \"web/\"]\n"));
    }

    #[test]
    fn registers_the_package_once_and_declares_its_folder() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE_NAME), TEMPLATE).unwrap();

        frontend(dir.path(), "clients/web").unwrap();
        frontend(dir.path(), "clients/web").unwrap();

        let manifest = crate::manifest::load(&dir.path().join(FILE_NAME)).unwrap();
        assert_eq!(
            manifest.products["app"].frontend.iter().collect::<Vec<_>>(),
            ["clients/web"]
        );
        let root = manifest.workspace.root.unwrap();
        assert_eq!(root.iter().filter(|p| *p == "clients/").count(), 1);
    }
}
