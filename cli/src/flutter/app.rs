//! `skies g flutter-app`: an ordinary `flutter create` app with the Skies spine and i18n wiring added.
//!
//! Flutter's own generator stays the source of the project; Skies adds only what its conventions assume: the
//! `skies_flutter` runtime pinned to this binary's version (every Skies package is released in lockstep, like the
//! `@skiesjs/*` pins of `g web-app`), gen_l10n fed from per-feature ARB catalogs, and a first shared catalog so the
//! localization build has a template to start from. Inside a Skies application the package is declared the way
//! `g web-app` declares a web package: a product's `frontend` and the root allowlist in `Skies.toml` (so `skies
//! doctor` checks it) and a job in the template CI. Nothing here is enforcement.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use super::i18n::{self, FEATURES_DIR};
use super::names::{is_package_name, snake};
use crate::manifest::Project;
use crate::web::scaffold::{relative_path, run, write_new};
use crate::web::{ci, register};

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
    let parent = parent.canonicalize()?;
    let dir = parent.join(dir.file_name().unwrap_or(name.as_ref()));
    let parent = parent.as_path();
    let project = Project::discover(&dir).ok();

    // A failed step (Flutter missing, the pinned spine not on pub.dev yet, no network) leaves no half-made package:
    // the folder was absent or empty, and the manifest is only touched once the package exists.
    if let Err(error) = scaffold(&dir, parent, &package) {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(error.context(format!("g flutter-app failed; removed {}", dir.display())));
    }

    let package_path = project.as_ref().map_or_else(
        || dir.display().to_string(),
        |p| relative_path(&p.root, &dir).trim_start_matches("./").to_string(),
    );
    let (setup, command) = flutter_runner(&package_path);
    if let Some(project) = &project {
        register::frontend(&project.root, &package_path)?;
        register::runner(&project.root, "flutter", &setup, &command)?;
        ci::add_flutter_package(&project.root, &package_path, &package)?;
    }
    println!("\ncreated Flutter app {package} in {}", dir.display());
    println!("next: `skies g feature <Name> --package {package_path}`");
    if project.is_none() {
        println!(
            "\nTo run Flutter specs (.specs/<id>/e2e/*_test.dart) with `skies proof`, declare a runner in Skies.toml:\n\n\
             [runners.flutter]\nsetup = {}\ncommand = {}",
            toml::Value::String(setup),
            toml::Value::String(command)
        );
    }
    Ok(0)
}

/// `flutter create`, the spine and localization dependencies, and the harness files, in `dir`.
fn scaffold(dir: &Path, parent: &Path, package: &str) -> Result<()> {
    let flutter = flutter_bin();
    let target = dir.to_string_lossy().into_owned();
    run(
        &flutter,
        &["create", "--project-name", package, "--empty", &target],
        parent,
    )?;
    let spine = spine_dependency();
    run(
        &flutter,
        &[
            "pub",
            "add",
            &spine,
            "intl:any",
            "flutter_localizations:{\"sdk\":\"flutter\"}",
        ],
        dir,
    )?;
    // `intl` must be the version flutter_localizations pins, so pub picks it; the pubspec then records that choice
    // as a caret constraint instead of leaving `any`.
    pin_resolved(dir, "intl")?;
    enable_generate(&dir.join("pubspec.yaml"))?;
    ignore_spec_copies(&dir.join(".gitignore"))?;
    write_new(&harness_files(dir))?;
    i18n::assemble(dir)?;
    run(&flutter, &["pub", "get"], dir)?;
    Ok(())
}

/// Replaces `<dependency>: any` in the pubspec with `^<version>`, the version pubspec.lock resolved.
fn pin_resolved(dir: &Path, dependency: &str) -> Result<()> {
    let lock = std::fs::read_to_string(dir.join("pubspec.lock"))?;
    let Some(version) = locked_version(&lock, dependency) else {
        bail!("pubspec.lock resolved no {dependency}");
    };
    let pubspec = dir.join("pubspec.yaml");
    let text = std::fs::read_to_string(&pubspec)?;
    let loose = format!("\n  {dependency}: any\n");
    if text.contains(&loose) {
        std::fs::write(
            &pubspec,
            text.replacen(&loose, &format!("\n  {dependency}: ^{version}\n"), 1),
        )?;
    }
    Ok(())
}

/// The version pubspec.lock records for a package (`packages: > intl: > version: "0.20.2"`).
fn locked_version(lock: &str, dependency: &str) -> Option<String> {
    let entry = lock.find(&format!("\n  {dependency}:\n"))?;
    lock[entry..]
        .lines()
        .skip(2)
        .take_while(|line| line.starts_with("    ") || line.is_empty())
        .find_map(|line| line.trim().strip_prefix("version: "))
        .map(|version| version.trim_matches('"').to_string())
}

/// `skies_flutter` at exactly this binary's version: the Dart spine, the Roslyn doctor, and the `@skiesjs/*`
/// packages are released together, so an app never mixes majors (an unpinned add takes pub.dev's latest).
fn spine_dependency() -> String {
    format!("skies_flutter:{}", crate::dotnet::FRAMEWORK_VERSION)
}

/// The `[runners.flutter]` for headless spec cases (docs/FLUTTER-CONVENTIONS.md): copy the spec's `e2e/` into the
/// package's ignored `test/.skies_spec/`, where `package:<app>/` imports resolve, run it with Dart's JSON reporter,
/// and convert that to JUnit. The `;` is deliberate: a red run fails its tests and must still write its report.
fn flutter_runner(package: &str) -> (String, String) {
    (
        "flutter pub global activate junitreport".to_string(),
        format!(
            "rm -rf {package}/test/.skies_spec && mkdir -p {package}/test/.skies_spec && cp -R {{dir}}/. \
             {package}/test/.skies_spec/ && cd {package} && flutter test test/.skies_spec --file-reporter \
             json:.dart_tool/skies_spec.json; flutter pub global run junitreport:tojunit --input \
             .dart_tool/skies_spec.json --output {{report}}"
        ),
    )
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
    fn the_spine_is_pinned_to_this_binarys_version() {
        assert_eq!(
            spine_dependency(),
            format!("skies_flutter:{}", env!("CARGO_PKG_VERSION"))
        );
        let pubspec = include_str!("../../../flutter-sdk/packages/skies_flutter/pubspec.yaml");
        assert!(
            pubspec.contains(&format!("\nversion: {}\n", env!("CARGO_PKG_VERSION"))),
            "the pin names a version the spine is released at"
        );
    }

    #[test]
    fn the_locked_version_is_read_from_its_own_entry() {
        let lock = "packages:\n  http:\n    dependency: transitive\n    description:\n      name: http\n    \
                    version: \"1.2.0\"\n  intl:\n    dependency: \"direct main\"\n    description:\n      name: intl\n      \
                    url: \"https://pub.dev\"\n    source: hosted\n    version: \"0.20.2\"\nsdks:\n  dart: \">=3.0.0\"\n";
        assert_eq!(locked_version(lock, "intl").as_deref(), Some("0.20.2"));
        assert_eq!(locked_version(lock, "http").as_deref(), Some("1.2.0"));
        assert_eq!(locked_version(lock, "missing"), None);
    }

    #[test]
    fn the_runner_copies_a_spec_into_the_package_and_writes_junit() {
        let (setup, command) = flutter_runner("clients/mobile");
        assert_eq!(setup, "flutter pub global activate junitreport");
        assert!(
            command.starts_with("rm -rf clients/mobile/test/.skies_spec && mkdir -p clients/mobile/test/.skies_spec")
        );
        assert!(
            command.contains("cp -R {dir}/. clients/mobile/test/.skies_spec/ && cd clients/mobile && flutter test")
        );
        assert!(command.ends_with("junitreport:tojunit --input .dart_tool/skies_spec.json --output {report}"));
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
