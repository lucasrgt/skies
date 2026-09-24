//! Imports of Skies 4 packages that no longer exist.
//!
//! Two are mechanical: the pre-scope package names (`skies-react`, `eslint-plugin-skies`) become the scoped packages
//! they turned into, and the frontend SDK's Playwright fixture becomes plain `@playwright/test` (the fixture only
//! added a console-error watcher on top of it). The rest belonged to the Skies 4 proof layer: the backend ledgers
//! that tied browser and device journeys to backend slices, and the Assay adapter. Skies 5 has no replacement,
//! because a feature's E2E now lives in its spec, so the migration names each test that imports them and leaves
//! the decision (delete it, or rewrite it as a spec) to a person.

use std::path::Path;

use super::Plan;

/// The names the removed frontend SDK was installed under: its package name and the alias Skies 4 templates used.
const SDK_NAMES: &[&str] = &["@skiesjs/frontend-sdk", "skies-frontend-sdk"];

/// Rewrites what can be rewritten and reports the rest. Returns the new text when anything changed.
pub fn rewrite(root: &Path, path: &Path, text: &str, plan: &mut Plan) -> Option<String> {
    let mut updated = rename_packages(text);
    for sdk in SDK_NAMES {
        updated = replace_specifier(&updated, &format!("{sdk}/playwright"), "@playwright/test");
    }
    let relative = path.strip_prefix(root).unwrap_or(path).display().to_string();
    if SDK_NAMES.iter().any(|sdk| imports_package(&updated, sdk)) {
        plan.follow_up_file(
            "imports the removed Skies 4 frontend SDK (backend ledger, Assay adapter, doctor); delete the test or \
             rewrite it as a spec",
            &relative,
        );
    }
    if uses(&updated, "package:skies_flutter/skies_flutter_testing.dart") {
        plan.follow_up_file(
            "imports the removed Skies 4 Dio backend ledger (skies_flutter_testing.dart); delete the test or rewrite \
             it as a spec",
            &relative,
        );
    }
    (updated != text).then_some(updated)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renames_pre_scope_imports_only_as_whole_specifiers() {
        let text = "import { useSession } from \"skies-react\";\nconst skies = require('eslint-plugin-skies');\nconst x = \"skies-react-extra\";\n";
        assert_eq!(
            rename_packages(text),
            "import { useSession } from \"@skiesjs/react\";\nconst skies = require('@skiesjs/eslint-plugin');\nconst x = \"skies-react-extra\";\n"
        );
    }

    #[test]
    fn the_playwright_fixture_becomes_playwright_and_the_ledger_is_reported() {
        let text = "import { test } from \"@skiesjs/frontend-sdk/playwright\";\nimport { observeBackend } from \"@skiesjs/frontend-sdk/playwright-backend\";\n";
        let mut plan = Plan::default();

        let out = rewrite(Path::new("/r"), Path::new("/r/web/e2e/pay.spec.ts"), text, &mut plan).unwrap();

        assert_eq!(
            out,
            "import { test } from \"@playwright/test\";\nimport { observeBackend } from \"@skiesjs/frontend-sdk/playwright-backend\";\n"
        );
        assert_eq!(
            plan.follow_ups.values().flatten().collect::<Vec<_>>(),
            ["web/e2e/pay.spec.ts"]
        );
        assert!(plan.changes.is_empty(), "nothing is copied into the app");
    }

    #[test]
    fn the_dio_ledger_and_the_assay_adapter_are_reported() {
        let mut plan = Plan::default();
        let dart = "import 'package:skies_flutter/skies_flutter_testing.dart';\n";
        let assay = "import { productVerification } from \"skies-frontend-sdk/product-verification\";\n";

        assert!(
            rewrite(
                Path::new("/r"),
                Path::new("/r/app/integration_test/a_test.dart"),
                dart,
                &mut plan
            )
            .is_none()
        );
        assert!(
            rewrite(
                Path::new("/r"),
                Path::new("/r/web/src/Pay.assay.test.tsx"),
                assay,
                &mut plan
            )
            .is_none()
        );

        assert_eq!(plan.follow_ups.len(), 2);
    }
}
