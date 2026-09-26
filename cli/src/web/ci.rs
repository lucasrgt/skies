//! Adds a new frontend package to the CI workflow `skies new` writes (`.github/workflows/ci.yml`).
//!
//! Plain YAML a reviewer reads in the diff. A web package gets its `npm ci` before `skies doctor` (the doctor lints
//! and typechecks it, which needs its dependencies) and a job that builds it and runs its `test` script. A Flutter
//! package gets a job that analyzes it and runs the spec cases that import it (the doctor's SKYFL rules are native,
//! so the doctor step needs nothing from it). A workflow without the template's `skies doctor` step is the owner's;
//! it is left alone and the steps to add are printed.

use std::path::Path;

use anyhow::{Context, Result};

pub const WORKFLOW: &str = ".github/workflows/ci.yml";

const DOCTOR_STEP: &str = "      - run: skies doctor";

/// Wires `package` (relative to `root`, forward slashes) into the app's CI workflow when it has the template's.
pub fn add_package(root: &Path, package: &str) -> Result<()> {
    let path = root.join(WORKFLOW);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(());
    };
    match with_package(&text, package) {
        Some(updated) if updated != text => {
            std::fs::write(&path, updated).with_context(|| format!("writing {}", path.display()))?;
            println!("added {package} to {WORKFLOW}: its `npm ci` before `skies doctor`, and a build-and-test job");
        }
        Some(_) => {}
        None => println!(
            "note: {WORKFLOW} has no `skies doctor` step; run `npm ci --prefix {package}` before the doctor, and \
             `npm run build` and `npm test` in {package}"
        ),
    }
    Ok(())
}

/// The workflow with `package` wired in; unchanged when it already is, `None` without the doctor step.
pub fn with_package(text: &str, package: &str) -> Option<String> {
    let install = format!("      - run: npm ci --prefix {package}");
    if text.lines().any(|line| line == install) {
        return Some(text.to_string());
    }
    let at = text.lines().position(|line| line == DOCTOR_STEP)?;
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    lines.insert(at, install);
    let job = job_id(package);
    let mut out = lines.join("\n") + "\n";
    out.push_str(&format!(
        "\n  web-{job}:\n    name: web ({package})\n    runs-on: ubuntu-latest\n    defaults:\n      run:\n        \
         working-directory: {package}\n    steps:\n      - uses: actions/checkout@v7\n      - uses: actions/setup-node@v7\n        \
         with:\n          node-version: \"24\"\n      - run: npm ci\n      - run: npm run build\n      - run: npm test\n"
    ));
    Some(out)
}

/// Wires the Flutter package `package` (relative to `root`, forward slashes), whose Dart name is `dart`, into the
/// app's CI workflow when it has the template's.
pub fn add_flutter_package(root: &Path, package: &str, dart: &str) -> Result<()> {
    let path = root.join(WORKFLOW);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(());
    };
    match with_flutter_package(&text, package, dart) {
        Some(updated) if updated != text => {
            std::fs::write(&path, updated).with_context(|| format!("writing {}", path.display()))?;
            println!("added {package} to {WORKFLOW}: a job that analyzes it and runs the spec cases importing it");
        }
        Some(_) => {}
        None => println!(
            "note: {WORKFLOW} has no `skies doctor` step; add a job that runs `flutter pub get` and `flutter analyze` \
             in {package}"
        ),
    }
    Ok(())
}

/// The workflow with a job for the Flutter package appended; unchanged when it has one, `None` without the doctor
/// step. Every case lives in a spec, so the job copies each spec's `e2e/` holding a case that imports
/// `package:<dart>/` into the package's ignored `test/.skies_spec/` (where that import resolves, as the Flutter
/// runner does per spec) and runs them there.
pub fn with_flutter_package(text: &str, package: &str, dart: &str) -> Option<String> {
    let job = format!("  flutter-{}:", job_id(package));
    if text.lines().any(|line| line == job) {
        return Some(text.to_string());
    }
    text.lines().position(|line| line == DOCTOR_STEP)?;
    let mut out = text.trim_end().to_string() + "\n";
    out.push_str(&format!(
        "\n{job}\n    name: flutter ({package})\n    runs-on: ubuntu-latest\n    defaults:\n      run:\n        \
         working-directory: {package}\n    steps:\n      - uses: actions/checkout@v7\n      \
         - uses: subosito/flutter-action@v2\n        with:\n          channel: stable\n          cache: true\n      \
         - run: flutter pub get\n      - run: flutter analyze\n      - name: spec cases importing package:{dart}\n        \
         run: |\n          rm -rf test/.skies_spec\n          found=0\n          \
         for e2e in \"$GITHUB_WORKSPACE\"/.specs/*/e2e; do\n            \
         grep -rqs --include='*_test.dart' \"package:{dart}/\" \"$e2e\" || continue\n            \
         spec=$(basename \"$(dirname \"$e2e\")\")\n            \
         mkdir -p \"test/.skies_spec/$spec\" && cp -R \"$e2e/.\" \"test/.skies_spec/$spec/\"\n            found=1\n          \
         done\n          \
         if [ \"$found\" = 1 ]; then flutter test test/.skies_spec; else echo \"no spec case imports package:{dart} yet\"; fi\n"
    ));
    Some(out)
}

/// A job id from a package path: lowercase alphanumerics, everything else a dash.
fn job_id(package: &str) -> String {
    package
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEMPLATE: &str = include_str!("../../templates/app/.github/workflows/ci.yml");

    #[test]
    fn the_package_installs_before_the_doctor_and_gets_its_own_job() {
        let out = with_package(TEMPLATE, "clients/web").unwrap();
        assert!(out.contains("      - run: npm ci --prefix clients/web\n      - run: skies doctor\n"));
        assert!(out.contains("\n  web-clients-web:\n    name: web (clients/web)\n"));
        assert!(out.contains("        working-directory: clients/web\n"));
        assert!(out.ends_with("      - run: npm ci\n      - run: npm run build\n      - run: npm test\n"));
        assert!(out.contains("actions/setup-node@v7\n        with:\n          node-version: \"24\"\n"));
        assert_eq!(
            with_package(&out, "clients/web").unwrap(),
            out,
            "a second run changes nothing"
        );
        assert!(with_package("jobs: {}\n", "web").is_none());
    }

    #[test]
    fn a_flutter_package_gets_an_analyze_and_spec_job_once() {
        let out = with_flutter_package(TEMPLATE, "clients/mobile", "mobile").unwrap();
        assert!(out.starts_with(TEMPLATE.trim_end()));
        assert!(out.contains("\n  flutter-clients-mobile:\n    name: flutter (clients/mobile)\n"));
        assert!(out.contains("        working-directory: clients/mobile\n"));
        assert!(out.contains("      - run: flutter pub get\n      - run: flutter analyze\n"));
        assert!(out.contains("\"package:mobile/\""));
        assert!(out.contains("then flutter test test/.skies_spec;"));
        assert!(
            !out.contains("npm ci --prefix clients/mobile"),
            "the doctor needs nothing installed"
        );
        assert_eq!(with_flutter_package(&out, "clients/mobile", "mobile").unwrap(), out);
        assert!(with_flutter_package("jobs: {}\n", "mobile", "mobile").is_none());
    }

    #[test]
    fn the_template_runs_build_test_and_the_doctor_at_this_version() {
        assert!(TEMPLATE.contains("      - run: dotnet test --no-build\n"));
        assert!(TEMPLATE.contains(DOCTOR_STEP));
        assert!(
            TEMPLATE.contains("actions/setup-node@v7\n        with:\n          node-version: \"24\"\n"),
            "the Node the framework's own CI runs"
        );
        let pin = format!(
            "npm install --global @skiesjs/cli@{}\n",
            crate::dotnet::FRAMEWORK_VERSION
        );
        assert!(
            TEMPLATE.contains(&pin),
            "the CI installs the skies binary this template ships with (tools/set-version.sh)"
        );
    }
}
