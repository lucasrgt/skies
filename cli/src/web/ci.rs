//! Adds a new web package to the CI workflow `skies new` writes (`.github/workflows/ci.yml`).
//!
//! Two lines of change, both plain YAML a reviewer reads in the diff: the package's `npm ci` before `skies doctor`
//! (the doctor lints and typechecks it, which needs its dependencies), and a job that builds the package and runs
//! its `test` script. A workflow without the template's `skies doctor` step is the owner's; it is left alone and the
//! step to add is printed.

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
    let job = package
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let mut out = lines.join("\n") + "\n";
    out.push_str(&format!(
        "\n  web-{job}:\n    name: web ({package})\n    runs-on: ubuntu-latest\n    defaults:\n      run:\n        \
         working-directory: {package}\n    steps:\n      - uses: actions/checkout@v7\n      - uses: actions/setup-node@v6\n        \
         with:\n          node-version: \"22\"\n      - run: npm ci\n      - run: npm run build\n      - run: npm test\n"
    ));
    Some(out)
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
        assert_eq!(
            with_package(&out, "clients/web").unwrap(),
            out,
            "a second run changes nothing"
        );
        assert!(with_package("jobs: {}\n", "web").is_none());
    }

    #[test]
    fn the_template_runs_build_test_and_the_doctor_at_this_version() {
        assert!(TEMPLATE.contains("      - run: dotnet test --no-build\n"));
        assert!(TEMPLATE.contains(DOCTOR_STEP));
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
