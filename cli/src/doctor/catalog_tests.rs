//! One source of truth for which frontend rules exist. The code catalogs decide: `@skiesjs/eslint-plugin`'s
//! `index.cjs` (each rule tagged with its SKYFE code, the warn tier in `WARN_TIER`) and the Flutter doctor's `RULES`.
//! These tests pin the two convention catalogs to them, row for row and tier for tier, and hold the twin contract: a
//! number up to 036 names one rule in both ecosystems, so it exists in both or in neither, at the same tier.

use std::collections::BTreeMap;
use std::path::Path;

use crate::doctor::Severity;
use crate::flutter::rules::RULES;

fn repo(path: &str) -> String {
    let full = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(path);
    std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("{}: {e}", full.display()))
}

/// The catalog rows of a convention doc: code → whether the row is a warning (its cell opens with "Warning.").
fn doc_rows(doc: &str, prefix: &str) -> BTreeMap<String, bool> {
    doc.lines()
        .filter_map(|line| line.strip_prefix(&format!("| `{prefix}")))
        .map(|rest| {
            let (number, cell) = rest.split_once("` | ").expect("a catalog row has a code cell");
            (format!("{prefix}{number}"), cell.starts_with("Warning."))
        })
        .collect()
}

/// The SKYFE rules `index.cjs` registers: code → whether `recommended` sets it to warn.
fn eslint_rules() -> BTreeMap<String, bool> {
    let index = repo("frontend-sdk/packages/eslint-plugin/index.cjs");
    let warn_tier = index
        .split("const WARN_TIER = new Set([")
        .nth(1)
        .and_then(|rest| rest.split("]);").next())
        .expect("index.cjs declares WARN_TIER")
        .to_string();
    index
        .lines()
        .filter(|line| line.contains("require(\"./rules/"))
        .map(|line| {
            let name = line.trim().split('"').nth(1).expect("a quoted rule id");
            let code = line.rsplit("// ").next().expect("a SKYFE code comment").trim();
            (code.to_string(), warn_tier.contains(&format!("\"{name}\"")))
        })
        .collect()
}

fn flutter_rules() -> BTreeMap<String, bool> {
    RULES
        .iter()
        .map(|rule| (rule.code.to_string(), rule.severity == Severity::Warning))
        .collect()
}

#[test]
fn the_web_catalog_is_the_rules_the_plugin_registers() {
    let doc = doc_rows(&repo("docs/FRONTEND-CONVENTIONS.md"), "SKYFE");
    assert_eq!(
        doc.get("SKYFE023"),
        Some(&true),
        "the parse sees the rows and their tiers"
    );
    assert_eq!(
        doc,
        eslint_rules(),
        "FRONTEND-CONVENTIONS.md rows (code, warning) vs index.cjs"
    );
}

#[test]
fn the_flutter_catalog_is_the_rules_the_doctor_runs() {
    let doc = doc_rows(&repo("docs/FLUTTER-CONVENTIONS.md"), "SKYFL");
    assert_eq!(
        doc.get("SKYFL001"),
        Some(&false),
        "the parse sees the rows and their tiers"
    );
    assert_eq!(
        doc,
        flutter_rules(),
        "FLUTTER-CONVENTIONS.md rows (code, warning) vs RULES"
    );
}

#[test]
fn a_shared_number_is_one_rule_in_both_ecosystems_at_one_tier() {
    let web: BTreeMap<u32, bool> = eslint_rules()
        .into_iter()
        .map(|(code, warn)| (code["SKYFE".len()..].parse().unwrap(), warn))
        .collect();
    let flutter: BTreeMap<u32, bool> = flutter_rules()
        .into_iter()
        .map(|(code, warn)| (code["SKYFL".len()..].parse().unwrap(), warn))
        .filter(|(number, _)| *number <= 36)
        .collect();
    assert_eq!(web, flutter, "SKYFE/SKYFL twins (number → warning) must match");
}

#[test]
fn no_catalog_lists_a_rule_that_does_not_run() {
    for doc in [
        "docs/CONVENTIONS.md",
        "docs/FRONTEND-CONVENTIONS.md",
        "docs/FLUTTER-CONVENTIONS.md",
    ] {
        assert!(!repo(doc).contains("(planned)"), "{doc} lists a planned rule");
    }
}
