//! `skies g flutter-app`: an ordinary `flutter create` app with the Skies spine and i18n wiring added.
//!
//! Flutter's own generator stays the source of the project; Skies adds only what its conventions assume: the
//! `skies_flutter` runtime, gen_l10n fed from per-feature ARB catalogs, and a first shared catalog so the
//! localization build has a template to start from. Nothing here is enforcement; `skies doctor` finds the
//! package by its `pubspec.yaml`.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use super::i18n::{self, FEATURES_DIR};
use super::names::{is_package_name, snake};
use crate::web::scaffold::{run, write_new};

const L10N_YAML: &str =
    "arb-dir: lib/l10n\ntemplate-arb-file: app_en.arb\noutput-localization-file: app_localizations.dart\n";

/// The shared catalog every app starts with, in the one locale a new app speaks; `l10n.yaml` names it the template.
/// A language is added by adding its catalogs, and later feature scaffolds follow the set the app has.
const COMMON_COPY: (&str, &str) = (i18n::DEFAULT_LOCALE, "Skies app");

pub fn create(name: &str, path: Option<&Path>) -> Result<u8> {
    let package = snake(name);
    if !is_package_name(&package) {
        bail!("'{name}' does not make a Dart package name; start it with a letter");
    }
    let dir = std::env::current_dir()?.join(path.unwrap_or(Path::new(name)));
    if dir.exists() && std::fs::read_dir(&dir)?.next().is_some() {
        bail!("{} already exists and is not empty", dir.display());
    }
    let parent = dir.parent().unwrap_or(Path::new("/"));
    std::fs::create_dir_all(parent)?;

    let flutter = flutter_bin();
    let target = dir.to_string_lossy().into_owned();
    run(
        &flutter,
        &["create", "--project-name", &package, "--empty", &target],
        parent,
    )?;
    run(
        &flutter,
        &[
            "pub",
            "add",
            "skies_flutter",
            "intl:any",
            "flutter_localizations:{\"sdk\":\"flutter\"}",
        ],
        &dir,
    )?;
    enable_generate(&dir.join("pubspec.yaml"))?;
    ignore_spec_copies(&dir.join(".gitignore"))?;
    write_new(&harness_files(&dir))?;
    i18n::assemble(&dir)?;
    run(&flutter, &["pub", "get"], &dir)?;
    println!("\ncreated Flutter app {package} in {}", dir.display());
    println!("next: `skies g feature <Name> --package {}`", dir.display());
    Ok(0)
}

pub fn flutter_bin() -> String {
    std::env::var("FLUTTER_BIN").unwrap_or_else(|_| "flutter".into())
}

/// The files Skies adds to a fresh Flutter project.
pub fn harness_files(dir: &Path) -> Vec<(PathBuf, String)> {
    let mut files = vec![(dir.join("l10n.yaml"), L10N_YAML.to_string())];
    let (locale, title) = COMMON_COPY;
    let catalog = format!("{{\n  \"appTitle\": \"{title}\"\n}}\n");
    files.push((dir.join(FEATURES_DIR).join(format!("common_{locale}.arb")), catalog));
    files
}

/// Turns on gen_l10n (`flutter: generate: true`), inserting the key under the existing `flutter:` section.
fn enable_generate(pubspec: &Path) -> Result<()> {
    let text = std::fs::read_to_string(pubspec)?;
    std::fs::write(pubspec, with_generate(&text)?)?;
    Ok(())
}

/// Every test lives in a spec at the repository root, and a Dart case must import `package:<app>/...`, so the Flutter
/// runner copies a spec's cases into the package's hidden `.skies_spec/` folder to run them. The copy is regenerated
/// per run, so it stays out of version control (and the doctor skips hidden folders).
fn ignore_spec_copies(gitignore: &Path) -> Result<()> {
    let text = std::fs::read_to_string(gitignore).unwrap_or_default();
    std::fs::write(gitignore, with_spec_copies_ignored(&text))?;
    Ok(())
}

fn with_spec_copies_ignored(text: &str) -> String {
    if text.lines().any(|line| line.trim() == SPEC_COPIES) {
        return text.to_string();
    }
    let separator = if text.is_empty() || text.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    format!("{text}{separator}\n# Where the Skies flutter runner copies a spec's cases to run them.\n{SPEC_COPIES}\n")
}

const SPEC_COPIES: &str = ".skies_spec/";

pub fn with_generate(pubspec: &str) -> Result<String> {
    if pubspec.lines().any(|line| line.trim() == "generate: true") {
        return Ok(pubspec.to_string());
    }
    let mut out = String::new();
    let mut inserted = false;
    for line in pubspec.split_inclusive('\n') {
        out.push_str(line);
        if !inserted && line.trim_end() == "flutter:" {
            out.push_str("  generate: true\n");
            inserted = true;
        }
    }
    if !inserted {
        bail!("pubspec.yaml has no top-level flutter: section");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enables_gen_l10n_under_the_flutter_section_only() {
        let pubspec =
            "name: demo\ndependencies:\n  flutter:\n    sdk: flutter\n\nflutter:\n  uses-material-design: true\n";
        let out = with_generate(pubspec).unwrap();
        assert!(out.ends_with("flutter:\n  generate: true\n  uses-material-design: true\n"));
        assert_eq!(with_generate(&out).unwrap(), out);
        assert!(with_generate("name: demo\n").is_err());
    }

    #[test]
    fn spec_copies_are_ignored_once() {
        let out = with_spec_copies_ignored("build/\n");
        assert!(out.ends_with("\n.skies_spec/\n"));
        assert_eq!(with_spec_copies_ignored(&out), out);
        assert!(with_spec_copies_ignored("build/").starts_with("build/\n\n#"));
    }

    #[test]
    fn the_harness_is_l10n_wiring_without_gate_files() {
        let files = harness_files(Path::new("/app"));
        let names: Vec<String> = files.iter().map(|(p, _)| p.display().to_string()).collect();
        assert!(names.contains(&"/app/l10n.yaml".to_string()));
        assert!(names.contains(&"/app/lib/l10n/features/common_en.arb".to_string()));
        assert_eq!(names.len(), 2, "one locale until the app adds another: {names:?}");
        assert!(
            !names
                .iter()
                .any(|n| n.ends_with("package.json") || n.contains("flows.json"))
        );
        assert!(files[0].1.contains("template-arb-file: app_en.arb"));
    }
}
