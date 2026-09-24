//! The SKYFL architecture rules for Flutter packages, run natively by `skies doctor`.
//!
//! Rule ids keep their 4.x meaning: SKYFL001–035 mirror the SKYFE slots by position, so a code seen in an old
//! report still names the same concern, and SKYFL036 mirrors SKYFE036 (tests live in a spec). Skies 5 keeps only the architecture rules. Proof rules (a test, a tag, a
//! flow manifest must exist), endpoint coverage, and the design-token band are gone, and their ids stay unused
//! rather than being reassigned. SKYFL037–040 are the accessibility floor (see `a11y`): Flutter-specific, with no
//! SKYFE twin, because the web's floor is jsx-a11y.

mod a11y;
#[cfg(test)]
mod a11y_tests;
#[cfg(test)]
mod calibration_tests;
mod checks;
mod facts;
mod navigation;
mod syntax;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use rayon::prelude::*;

use crate::doctor::{Finding, Severity};
use crate::flutter::{files_with_extension, i18n};
use facts::Facts;

/// Every rule still enforced, by code. The gaps are the retired proof, coverage, and design rules.
pub const RULES: [(&str, &str); 30] = [
    ("SKYFL001", "view-purity"),
    ("SKYFL002", "data-door"),
    ("SKYFL003", "no-mock"),
    ("SKYFL004", "viewmodel-render-agnostic"),
    ("SKYFL007", "mandatory-state"),
    ("SKYFL009", "viewmodel-platform-agnostic"),
    ("SKYFL010", "state-completeness"),
    ("SKYFL011", "i18n-parity"),
    ("SKYFL013", "mutation-error-surface"),
    ("SKYFL014", "no-hardcoded-copy"),
    ("SKYFL015", "declarative-redirect"),
    ("SKYFL016", "session-one-door"),
    ("SKYFL017", "guard-tristate"),
    ("SKYFL018", "route-param-guard"),
    ("SKYFL019", "safe-back"),
    ("SKYFL020", "configured-base-url"),
    ("SKYFL021", "raw-html-one-door"),
    ("SKYFL022", "no-open-redirect"),
    ("SKYFL023", "no-placeholder"),
    ("SKYFL027", "mutation-defaults"),
    ("SKYFL028", "no-manual-refetch"),
    ("SKYFL029", "refresh-one-door"),
    ("SKYFL030", "typed-navigation"),
    ("SKYFL031", "submit-invalid-path"),
    ("SKYFL032", "field-error-surface"),
    ("SKYFL036", "tests-live-in-specs"),
    ("SKYFL037", "icon-button-label"),
    ("SKYFL038", "image-semantics"),
    ("SKYFL039", "tap-target-label"),
    ("SKYFL040", "text-field-label"),
];

/// The code for a rule name.
pub fn code(name: &str) -> &'static str {
    RULES
        .iter()
        .find(|(_, rule)| *rule == name)
        .map(|(code, _)| *code)
        .expect("every rule name is catalogued")
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
    /// Under `lib/ui/`: the app's design-system primitives.
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

/// Runs every rule over a Flutter package (`lib/`, `test/`, `integration_test/`, and its ARB catalogs). Hidden
/// folders are skipped, which is where a spec runner copies a spec's cases to run them inside the package.
pub fn diagnose(project: &Path) -> Result<Vec<Finding>> {
    let lib = project.join("lib");
    if !lib.is_dir() {
        bail!("Flutter lib directory not found: {}", lib.display());
    }
    // A generated client is the generator's output, not app code: its `*_view.dart` models are DTOs, not Views.
    if project.join(crate::flutter::client::MARKER).is_file() {
        return Ok(Vec::new());
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
    findings.sort();
    Ok(findings)
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
            Finding::new(code("i18n-parity"), Severity::Error, gap.path, None, message)
        })
        .collect())
}
