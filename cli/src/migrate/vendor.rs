//! Keeps Skies 4 test helpers working after their packages are gone: the Playwright fixtures and backend ledger
//! (`@skiesjs/frontend-sdk/playwright*`), the Assay adapter (`…/product-verification`), and the Dio backend ledger
//! (`skies_flutter_testing.dart`) are copied into the application that uses them, and the imports are rewritten to
//! the copies. The tests stay as they were; the helpers simply become the application's own test support.
//!
//! The same pass renames imports of the two pre-scope package names (`skies-react`, `eslint-plugin-skies`) to the
//! scoped packages they became, and reports any other import of the removed frontend SDK.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use super::Plan;

const HEADER: &str = "// Copied from Skies 4 by `skies migrate 5`. This file now belongs to the application.\n";

/// The names the removed frontend SDK was installed under: its package name and the alias Skies 4 templates used.
const SDK_NAMES: &[&str] = &["@skiesjs/frontend-sdk", "skies-frontend-sdk"];

struct Helper {
    /// The import specifier the application used, with `{sdk}` standing for any of [`SDK_NAMES`].
    specifier: &'static str,
    /// Where the copy lands, relative to the package root.
    target: &'static str,
    /// The files written for it: (relative to the package root, content).
    files: &'static [(&'static str, &'static str)],
}

const HELPERS: &[Helper] = &[
    Helper {
        specifier: "{sdk}/playwright-backend",
        target: "e2e/support/skies-playwright-backend.mjs",
        files: &[
            (
                "e2e/support/skies-playwright-backend.mjs",
                include_str!("../../templates/migrate/skies-playwright-backend.mjs"),
            ),
            (
                "e2e/support/skies-playwright-backend.d.mts",
                include_str!("../../templates/migrate/skies-playwright-backend.d.mts"),
            ),
        ],
    },
    Helper {
        specifier: "{sdk}/playwright",
        target: "e2e/support/skies-playwright.mjs",
        files: &[
            (
                "e2e/support/skies-playwright.mjs",
                include_str!("../../templates/migrate/skies-playwright.mjs"),
            ),
            (
                "e2e/support/skies-playwright.d.mts",
                include_str!("../../templates/migrate/skies-playwright.d.mts"),
            ),
        ],
    },
    Helper {
        specifier: "{sdk}/product-verification",
        target: "test-support/skies-product-verification.mjs",
        files: &[
            (
                "test-support/skies-product-verification.mjs",
                include_str!("../../templates/migrate/skies-product-verification.mjs"),
            ),
            (
                "test-support/skies-product-verification.d.mts",
                include_str!("../../templates/migrate/skies-product-verification.d.mts"),
            ),
        ],
    },
    Helper {
        specifier: "package:skies_flutter/skies_flutter_testing.dart",
        target: "test_support/skies_backend_ledger.dart",
        files: &[(
            "test_support/skies_backend_ledger.dart",
            include_str!("../../templates/migrate/skies_backend_ledger.dart"),
        )],
    },
];

/// Rewrites helper imports in one file and schedules the copies next to its package. Returns the new text when
/// anything changed.
pub fn rewrite(root: &Path, path: &Path, text: &str, plan: &mut Plan) -> Option<String> {
    let mut updated = rename_packages(text);
    let mut changed = updated != text;
    for (helper, specifier) in HELPERS
        .iter()
        .flat_map(|helper| specifiers(helper).map(move |s| (helper, s)))
    {
        if !uses(&updated, &specifier) {
            continue;
        }
        let Some(package) = package_root(root, path) else {
            plan.follow_up_file(
                "imports a removed Skies helper outside any package",
                &display(root, path),
            );
            continue;
        };
        let package = match plan.shared_homes.get(&package) {
            Some(home) if helper.target.ends_with(".dart") => home.clone(),
            _ => package,
        };
        let target = package.join(helper.target);
        let relative = relative_import(path.parent()?, &target);
        updated = replace_specifier(&updated, &specifier, &relative);
        changed = true;
        for (file, content) in helper.files {
            let destination = package.join(file);
            if plan.vendored.insert(destination.clone()) {
                plan.write(&destination, format!("{HEADER}{content}"));
            }
        }
    }
    if SDK_NAMES.iter().any(|sdk| imports_package(&updated, sdk)) {
        plan.follow_up_file(
            "imports a removed @skiesjs/frontend-sdk entry point that has no copy; replace or delete the import",
            &display(root, path),
        );
    }
    changed.then_some(updated)
}

/// A relative Dart import or export: `import '../../app-core/flutter/test_support/fixture.dart';`.
static DART_RELATIVE_IMPORT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?m)^\s*(?:import|export)\s+['"](\.\.?/[^'"]+)['"]"#).unwrap());

/// Which package's copy of the Dart ledger each package should import instead of its own.
///
/// Test support is sometimes shared across packages by relative import (a library's `test_support/` fixture used by
/// the apps' integration tests). A copy per package would then give the fixture and the test two distinct
/// `BackendOutcome` types that do not unify, so a package that imports test code from another package that also
/// needs the ledger uses that package's copy.
pub fn shared_homes(root: &Path, files: &[PathBuf]) -> BTreeMap<PathBuf, PathBuf> {
    let specifier = "package:skies_flutter/skies_flutter_testing.dart";
    let dart: Vec<(&PathBuf, String)> = files
        .iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "dart"))
        .filter_map(|path| Some((path, std::fs::read_to_string(path).ok()?)))
        .collect();
    let needs: BTreeSet<PathBuf> = dart
        .iter()
        .filter(|(_, text)| uses(text, specifier))
        .filter_map(|(path, _)| package_root(root, path))
        .collect();
    let mut homes = BTreeMap::new();
    for (path, text) in &dart {
        let Some(package) = package_root(root, path).filter(|package| needs.contains(package)) else {
            continue;
        };
        for captures in DART_RELATIVE_IMPORT.captures_iter(text) {
            let target = normalize(&path.parent().unwrap_or(root).join(&captures[1]));
            let other = package_root(root, &target).filter(|other| *other != package && needs.contains(other));
            if let Some(other) = other {
                homes.entry(package.clone()).or_insert(other);
            }
        }
    }
    // Follow chains (a → b → c) to the last package, stopping on a cycle.
    let direct = homes.clone();
    for home in homes.values_mut() {
        let mut seen = BTreeSet::new();
        while let Some(next) = direct.get(home).filter(|next| seen.insert((*next).clone())) {
            *home = next.clone();
        }
    }
    homes.retain(|package, home| package != home);
    homes
}

/// Resolves `.` and `..` without touching the disk.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

fn specifiers(helper: &Helper) -> impl Iterator<Item = String> + '_ {
    let names: &[&str] = if helper.specifier.contains("{sdk}") {
        SDK_NAMES
    } else {
        &[""]
    };
    names.iter().map(|sdk| helper.specifier.replace("{sdk}", sdk))
}

/// Rewrites `"skies-react"`/`"eslint-plugin-skies"` (and their subpaths) to the scoped names.
fn rename_packages(text: &str) -> String {
    let mut out = text.to_string();
    for (old, new) in super::package_json::RENAMED {
        if !out.contains(old) {
            continue;
        }
        for quote in ['"', '\'', '`'] {
            out = out
                .replace(&format!("{quote}{old}{quote}"), &format!("{quote}{new}{quote}"))
                .replace(&format!("{quote}{old}/"), &format!("{quote}{new}/"));
        }
    }
    out
}

/// Whether `text` imports `package` or one of its subpaths as a quoted specifier.
fn imports_package(text: &str, package: &str) -> bool {
    ['"', '\'']
        .iter()
        .any(|quote| text.contains(&format!("{quote}{package}{quote}")) || text.contains(&format!("{quote}{package}/")))
}

/// Matches the specifier only as a whole quoted import, so `…/playwright` never matches `…/playwright-backend`.
fn uses(text: &str, specifier: &str) -> bool {
    text.contains(&format!("\"{specifier}\"")) || text.contains(&format!("'{specifier}'"))
}

fn replace_specifier(text: &str, specifier: &str, replacement: &str) -> String {
    text.replace(&format!("\"{specifier}\""), &format!("\"{replacement}\""))
        .replace(&format!("'{specifier}'"), &format!("'{replacement}'"))
}

/// The nearest ancestor holding a package manifest, without leaving the repository.
fn package_root(root: &Path, path: &Path) -> Option<PathBuf> {
    let mut dir = path.parent();
    while let Some(candidate) = dir {
        if candidate.join("package.json").is_file() || candidate.join("pubspec.yaml").is_file() {
            return Some(candidate.to_path_buf());
        }
        if candidate == root {
            return None;
        }
        dir = candidate.parent();
    }
    None
}

/// A `./`- or `../`-prefixed import path from `from_dir` to `target`, with forward slashes on every platform.
fn relative_import(from_dir: &Path, target: &Path) -> String {
    let from: Vec<_> = from_dir.components().collect();
    let to: Vec<_> = target.components().collect();
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<String> = std::iter::repeat_n("..".to_string(), from.len() - common).collect();
    parts.extend(
        to[common..]
            .iter()
            .map(|part| part.as_os_str().to_string_lossy().into_owned()),
    );
    let joined = parts.join("/");
    if joined.starts_with("..") {
        joined
    } else {
        format!("./{joined}")
    }
}

fn display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root).unwrap_or(path).display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_imports_walk_up_and_down() {
        assert_eq!(
            relative_import(Path::new("/r/app/e2e"), Path::new("/r/app/e2e/support/x.mjs")),
            "./support/x.mjs"
        );
        assert_eq!(
            relative_import(
                Path::new("/r/app/integration_test/support"),
                Path::new("/r/app/test_support/l.dart")
            ),
            "../../test_support/l.dart"
        );
    }

    #[test]
    fn renames_pre_scope_imports_only_as_whole_specifiers() {
        let text = "import { useSession } from \"skies-react\";\nconst skies = require('eslint-plugin-skies');\nconst x = \"skies-react-extra\";\n";
        assert_eq!(
            rename_packages(text),
            "import { useSession } from \"@skiesjs/react\";\nconst skies = require('@skiesjs/eslint-plugin');\nconst x = \"skies-react-extra\";\n"
        );
    }

    #[test]
    fn copies_aliased_helpers_and_reports_imports_without_a_copy() {
        let dir = tempfile::tempdir().unwrap();
        let package = dir.path().join("web");
        std::fs::create_dir_all(package.join("src/pay")).unwrap();
        std::fs::write(package.join("package.json"), "{}").unwrap();
        let test = package.join("src/pay/Pay.assay.test.tsx");
        let text = "import { productVerification } from \"skies-frontend-sdk/product-verification\";\nimport { doctor } from \"@skiesjs/frontend-sdk/doctor\";\n";
        let mut plan = Plan::default();

        let out = rewrite(dir.path(), &test, text, &mut plan).unwrap();

        assert!(
            out.starts_with(
                "import { productVerification } from \"../../test-support/skies-product-verification.mjs\";"
            )
        );
        assert_eq!(plan.changes.len(), 2);
        assert!(plan.follow_ups.keys().any(|note| note.contains("has no copy")));
    }

    #[test]
    fn a_package_that_imports_another_package_s_test_support_shares_its_ledger() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let testing = "import 'package:skies_flutter/skies_flutter_testing.dart';\n";
        let files = [
            ("core/pubspec.yaml", String::new()),
            ("core/test_support/fixture.dart", testing.to_string()),
            ("app/pubspec.yaml", String::new()),
            (
                "app/integration_test/support/harness.dart",
                "export '../../../core/test_support/fixture.dart';\n".into(),
            ),
            ("app/integration_test/flow_test.dart", testing.to_string()),
            ("solo/pubspec.yaml", String::new()),
            ("solo/integration_test/flow_test.dart", testing.to_string()),
        ];
        let mut paths = Vec::new();
        for (file, content) in files {
            let path = root.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, content).unwrap();
            paths.push(path);
        }
        let mut plan = Plan {
            shared_homes: shared_homes(root, &paths),
            ..Plan::default()
        };
        assert_eq!(
            plan.shared_homes,
            BTreeMap::from([(root.join("app"), root.join("core"))])
        );

        let out = rewrite(
            root,
            &root.join("app/integration_test/flow_test.dart"),
            testing,
            &mut plan,
        )
        .unwrap();

        assert_eq!(out, "import '../../core/test_support/skies_backend_ledger.dart';\n");
    }

    #[test]
    fn copies_the_helper_into_the_package_and_rewrites_the_import() {
        let dir = tempfile::tempdir().unwrap();
        let package = dir.path().join("clients/web");
        std::fs::create_dir_all(package.join("e2e")).unwrap();
        std::fs::write(package.join("package.json"), "{}").unwrap();
        let spec = package.join("e2e/pay.spec.ts");
        let text = "import { test } from \"@skiesjs/frontend-sdk/playwright\";\nimport { observeBackend } from \"@skiesjs/frontend-sdk/playwright-backend\";\n";
        let mut plan = Plan::default();

        let out = rewrite(dir.path(), &spec, text, &mut plan).unwrap();

        assert_eq!(
            out,
            "import { test } from \"./support/skies-playwright.mjs\";\nimport { observeBackend } from \"./support/skies-playwright-backend.mjs\";\n"
        );
        assert_eq!(plan.changes.len(), 4);
    }
}
