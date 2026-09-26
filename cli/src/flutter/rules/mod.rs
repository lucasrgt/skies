//! The SKYFL architecture rules for Flutter packages, run natively by `skies doctor`.
//!
//! A number up to 036 is the SKYFE rule of the same number: the same intent and tier in Flutter's spelling (the
//! twin contract `doctor::catalog_tests` pins), so a code names one concern in either ecosystem. Skies 5 keeps only
//! the architecture rules. Proof rules (a test, a tag, a flow manifest must exist), endpoint coverage, and the
//! design-token band are gone, and their ids stay unused rather than being reassigned. SKYFL037–040 are the
//! accessibility floor (see `a11y`): Flutter-specific, with no SKYFE twin, because the web's floor is jsx-a11y. A
//! finding can be silenced only by a reasoned `skies-ignore` directive, which the report shows (see `suppress`).

mod a11y;
#[cfg(test)]
mod a11y_tests;
#[cfg(test)]
mod calibration_tests;
mod checks;
mod facts;
mod navigation;
mod session;
mod suppress;
mod syntax;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod twin_tests;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use rayon::prelude::*;

use crate::doctor::{Finding, Severity, Suppressed};
use crate::flutter::{files_with_extension, i18n};
use facts::Facts;

/// One catalogued rule: its code, its name, and its tier. The tier lives here, not at the call that reports it, so
/// the catalog (and the docs pinned to it by `catalog_tests`) is the one place a rule's loudness is decided: an
/// architecture or security rule is an error; a taste, or a heuristic that cannot see everything, is a warning.
pub struct Rule {
    pub code: &'static str,
    pub name: &'static str,
    pub severity: Severity,
}

const fn error(code: &'static str, name: &'static str) -> Rule {
    Rule {
        code,
        name,
        severity: Severity::Error,
    }
}

const fn warning(code: &'static str, name: &'static str) -> Rule {
    Rule {
        code,
        name,
        severity: Severity::Warning,
    }
}

/// Every rule still enforced, by code. The gaps are the retired proof, coverage, and design rules.
pub const RULES: [Rule; 29] = [
    error("SKYFL001", "view-purity"),
    error("SKYFL002", "data-door"),
    error("SKYFL003", "no-mock"),
    error("SKYFL004", "viewmodel-render-agnostic"),
    error("SKYFL007", "mandatory-state"),
    error("SKYFL010", "state-completeness"),
    error("SKYFL011", "i18n-parity"),
    error("SKYFL013", "mutation-error-surface"),
    error("SKYFL014", "no-hardcoded-copy"),
    error("SKYFL015", "declarative-redirect"),
    error("SKYFL016", "session-one-door"),
    error("SKYFL017", "guard-tristate"),
    error("SKYFL018", "route-param-guard"),
    error("SKYFL019", "safe-back"),
    error("SKYFL020", "configured-base-url"),
    error("SKYFL021", "raw-html-one-door"),
    error("SKYFL022", "no-open-redirect"),
    warning("SKYFL023", "no-placeholder"),
    error("SKYFL027", "mutation-defaults"),
    warning("SKYFL028", "no-manual-refetch"),
    error("SKYFL029", "refresh-one-door"),
    error("SKYFL030", "typed-navigation"),
    warning("SKYFL031", "submit-invalid-path"),
    warning("SKYFL032", "field-error-surface"),
    error("SKYFL036", "tests-live-in-specs"),
    error("SKYFL037", "icon-button-label"),
    error("SKYFL038", "image-semantics"),
    warning("SKYFL039", "tap-target-label"),
    warning("SKYFL040", "text-field-label"),
];

fn rule(name: &str) -> &'static Rule {
    RULES
        .iter()
        .find(|rule| rule.name == name)
        .expect("every rule name is catalogued")
}

/// A finding of the named rule, at the rule's catalogued tier.
pub fn finding(name: &str, file: PathBuf, line: Option<usize>, message: String) -> Finding {
    let rule = rule(name);
    Finding::new(rule.code, rule.severity, file, line, message)
}

/// What a file is, by the conventions its path encodes.
#[derive(Debug, Clone, Copy, Default)]
pub struct Role {
    /// Under `lib/`, as opposed to `test/`.
    pub lib: bool,
    /// Test code: anything under `test/` or `integration_test/` (a harness, a support file, a fake) or a
    /// `*_test.dart`. The architecture rules police the shipped app; only SKYFL036 reads test code.
    pub test: bool,
    pub view: bool,
    pub model: bool,
    /// A `part of` a View or a ViewModel: the same library, so the same rules (a View part holds copy, a ViewModel
    /// part may call the generated client).
    pub view_part: bool,
    pub model_part: bool,
    /// Under `lib/ui/`: the app's design-system primitives (the `AppInput` field every form uses).
    pub ui: bool,
    /// `lib/session.dart`, `lib/skies_client.dart`, `lib/guard(s).dart`: the seams allowed to touch tokens.
    pub session_door: bool,
    /// `lib/html.dart` or `lib/html/`: the one place raw HTML may render.
    pub html_door: bool,
    /// The file name mentions a route or guard.
    pub routing: bool,
}

impl Role {
    pub fn of(relative: &str) -> Role {
        let name = relative.rsplit('/').next().unwrap_or(relative);
        let in_lib = |rest: &str| relative.strip_prefix("lib/").is_some_and(|r| r == rest);
        Role {
            lib: relative.starts_with("lib/"),
            test: name.ends_with("_test.dart")
                || relative.starts_with("test/")
                || relative.starts_with("integration_test/"),
            view: name.ends_with("_view.dart"),
            model: name.ends_with("_view_model.dart"),
            ui: relative.starts_with("lib/ui/") || relative.contains("/lib/ui/"),
            session_door: ["session.dart", "skies_client.dart", "guard.dart", "guards.dart"]
                .iter()
                .any(|f| in_lib(f)),
            html_door: relative.starts_with("lib/html/") || relative == "lib/html.dart",
            routing: name.contains("route") || name.contains("guard"),
            ..Role::default()
        }
    }

    /// The role of a file that declares `part of '<library>'`: its own, plus the library's View/ViewModel kind.
    fn with_part_of(mut self, library: Option<&str>) -> Role {
        let name = library
            .map(|uri| uri.rsplit('/').next().unwrap_or(uri))
            .unwrap_or_default();
        self.model_part = name.ends_with("_view_model.dart");
        self.view_part = name.ends_with("_view.dart");
        self
    }
}

/// Generated sources the app does not write by hand: build_runner output, the l10n classes, and a generated API
/// client package (`packages/<name>_api/`). No rule polices them.
pub fn generated(path: &str) -> bool {
    path.ends_with(".g.dart")
        || path.ends_with(".freezed.dart")
        || path.contains("/lib/l10n/")
        || path
            .split('/')
            .collect::<Vec<_>>()
            .windows(2)
            .any(|pair| pair[0] == "packages" && pair[1].ends_with("_api"))
}

/// One parsed file, kept for the checks that look across files.
pub struct Source {
    pub path: PathBuf,
    pub role: Role,
    pub facts: Facts,
}

/// The findings that stand, and the ones a reasoned `skies-ignore` directive suppressed (see [`suppress`]).
#[derive(Debug, Default)]
pub struct Diagnosis {
    pub findings: Vec<Finding>,
    pub suppressed: Vec<Suppressed>,
}

/// The findings that stand over a Flutter package; see [`analyze`].
#[cfg(test)]
pub fn diagnose(project: &Path) -> Result<Vec<Finding>> {
    Ok(analyze(project)?.findings)
}

/// Runs every rule over a Flutter package (`lib/`, `test/`, `integration_test/`, and its ARB catalogs). Hidden
/// folders are skipped, which is where a spec runner copies a spec's cases to run them inside the package.
pub fn analyze(project: &Path) -> Result<Diagnosis> {
    let lib = project.join("lib");
    if !lib.is_dir() {
        bail!("Flutter lib directory not found: {}", lib.display());
    }
    // A generated client is the generator's output, not app code: its `*_view.dart` models are DTOs, not Views.
    if project.join(crate::flutter::client::MARKER).is_file() {
        return Ok(Diagnosis::default());
    }
    let mut paths = files_with_extension(&lib, "dart");
    paths.extend(files_with_extension(&project.join("test"), "dart"));
    paths.extend(files_with_extension(&project.join("integration_test"), "dart"));

    let sources: Vec<Source> = paths
        .into_par_iter()
        .filter_map(|path| {
            let text = std::fs::read_to_string(&path).ok()?;
            let relative = path
                .strip_prefix(project)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let facts = Facts::parse(&text);
            Some(Source {
                role: Role::of(&relative).with_part_of(facts.part_of.as_deref()),
                facts,
                path,
            })
        })
        .collect();
    let by_path: HashMap<&Path, &Source> = sources.iter().map(|s| (s.path.as_path(), s)).collect();

    let mut findings: Vec<Finding> = sources
        .par_iter()
        .flat_map_iter(|source| checks::file(source, &by_path).into_iter().chain(a11y::file(source)))
        .collect();
    findings.extend(checks::project(&sources));
    findings.extend(i18n_parity(project)?);
    let (mut findings, mut suppressed) = suppress::apply(findings, &sources);
    findings.sort();
    suppressed.sort_by(|a, b| a.finding.cmp(&b.finding));
    Ok(Diagnosis { findings, suppressed })
}

fn i18n_parity(project: &Path) -> Result<Vec<Finding>> {
    let mut catalogs = Vec::new();
    for root in [project.join("lib/l10n"), project.join("l10n")] {
        for path in i18n::find_arb(&root) {
            catalogs.push(i18n::read_catalog(&path)?);
        }
    }
    Ok(i18n::check_parity(&catalogs)
        .into_iter()
        .map(|gap| {
            let message = format!("missing ARB keys: {}", gap.missing.join(", "));
            finding("i18n-parity", gap.path, None, message)
        })
        .collect())
}
