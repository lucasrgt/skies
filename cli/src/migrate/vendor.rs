//! Keeps Skies 4 E2E helpers working after their packages are gone: the Playwright fixtures and backend ledger
//! (`@skiesjs/frontend-sdk/playwright*`) and the Dio backend ledger (`skies_flutter_testing.dart`) are copied into
//! the application that uses them, and the imports are rewritten to the copies. The tests stay as they were; the
//! helpers simply become the application's own test support.

use std::path::{Path, PathBuf};

use super::Plan;

const HEADER_TS: &str =
    "// Copied from Skies 4 by `skies migrate 5`. This file now belongs to the application.\n";
const HEADER_DART: &str =
    "// Copied from Skies 4 by `skies migrate 5`. This file now belongs to the application.\n";

struct Helper {
    /// The import specifier the application used.
    specifier: &'static str,
    /// Where the copy lands, relative to the package root.
    target: &'static str,
    /// The files written for it: (relative to the package root, content).
    files: &'static [(&'static str, &'static str)],
}

const HELPERS: &[Helper] = &[
    Helper {
        specifier: "@skiesjs/frontend-sdk/playwright-backend",
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
        specifier: "@skiesjs/frontend-sdk/playwright",
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
    let mut updated = text.to_string();
    let mut changed = false;
    for helper in HELPERS {
        if !uses(&updated, helper.specifier) {
            continue;
        }
        let Some(package) = package_root(root, path) else {
            plan.follow_up_file(
                "imports a removed Skies helper outside any package",
                &display(root, path),
            );
            continue;
        };
        let target = package.join(helper.target);
        let relative = relative_import(path.parent()?, &target);
        updated = replace_specifier(&updated, helper.specifier, &relative);
        changed = true;
        for (file, content) in helper.files {
            let destination = package.join(file);
            if plan.vendored.insert(destination.clone()) {
                let header = if file.ends_with(".dart") {
                    HEADER_DART
                } else {
                    HEADER_TS
                };
                plan.write(&destination, format!("{header}{content}"));
            }
        }
    }
    changed.then_some(updated)
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
    let mut parts: Vec<String> =
        std::iter::repeat_n("..".to_string(), from.len() - common).collect();
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
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_imports_walk_up_and_down() {
        assert_eq!(
            relative_import(
                Path::new("/r/app/e2e"),
                Path::new("/r/app/e2e/support/x.mjs")
            ),
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
