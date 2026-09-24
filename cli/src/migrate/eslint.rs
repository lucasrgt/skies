//! Removes ESLint settings for `skies/*` rules the 5.x plugin no longer has. ESLint fails the whole run on a
//! configured rule it cannot find, so a single leftover `"skies/test-colocated": "error"` would turn every lint red
//! after the upgrade. Rules the plugin still ships are left exactly as configured.

use std::sync::LazyLock;

use regex::Regex;

use super::Plan;

/// The rule ids `@skiesjs/eslint-plugin` 5.x ships, one per file under `frontend-sdk/packages/eslint-plugin/rules/`
/// (a unit test keeps the two in step).
const KEPT_RULES: &[&str] = &[
    "controller-field-state",
    "data-door",
    "guard-tristate",
    "i18n-completeness",
    "mandatory-state",
    "mutation-error-handled",
    "no-cast-navigation",
    "no-hardcoded-base-url",
    "no-hardcoded-copy",
    "no-manual-refetch-ritual",
    "no-mock",
    "no-open-redirect",
    "no-placeholder",
    "no-raw-html",
    "no-router-replace-in-effect",
    "query-client-defaults",
    "refresh-one-door",
    "route-param-guard",
    "safe-back",
    "session-one-door",
    "state-completeness",
    "submit-handles-invalid",
    "tests-live-in-specs",
    "view-purity",
    "viewmodel-render-agnostic",
];

/// A rule setting that opens its line: `"skies/x": …`, `'skies/x': …`, or YAML's `skies/x: …`.
static RULE_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s*["']?skies/([A-Za-z0-9-]+)["']?\s*:(.*)$"#).unwrap());

/// Whether a file name is an ESLint configuration the migration edits.
pub fn is_config(name: &str) -> bool {
    name.starts_with(".eslintrc")
        || ["js", "mjs", "cjs", "ts", "mts", "cts"]
            .iter()
            .any(|ext| name == format!("eslint.config.{ext}"))
}

pub fn migrate(text: &str, relative: &str, plan: &mut Plan) -> Option<String> {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let mut out = String::with_capacity(text.len());
    let mut removed = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let Some(captures) = RULE_LINE.captures(lines[index].trim_end()) else {
            out.push_str(lines[index]);
            index += 1;
            continue;
        };
        let rule = captures[1].to_string();
        if KEPT_RULES.contains(&rule.as_str()) {
            out.push_str(lines[index]);
            index += 1;
            continue;
        }
        // A value such as `["error", {` continues until its brackets close.
        let mut open = bracket_delta(&captures[2]);
        index += 1;
        while open > 0 && index < lines.len() {
            open += bracket_delta(lines[index]);
            index += 1;
        }
        removed.push(format!("skies/{rule}"));
    }
    if removed.is_empty() {
        return None;
    }
    removed.sort();
    removed.dedup();
    plan.follow_up_file(
        &format!(
            "removed ESLint settings for rules Skies 5 dropped: {}",
            removed.join(", ")
        ),
        relative,
    );
    let json = relative.ends_with(".json") || relative.ends_with(".eslintrc");
    Some(if json {
        super::package_json::fix_commas(&out)
    } else {
        out
    })
}

/// Net `[`/`{` nesting in a code fragment, ignoring quoted strings and a trailing `//` comment.
fn bracket_delta(fragment: &str) -> i32 {
    let (mut delta, mut quote, mut escaped) = (0, None::<char>, false);
    let mut previous = ' ';
    for character in fragment.chars() {
        if let Some(open) = quote {
            match character {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                _ if character == open => quote = None,
                _ => {}
            }
            continue;
        }
        match character {
            '/' if previous == '/' => break,
            '"' | '\'' | '`' => quote = Some(character),
            '[' | '{' | '(' => delta += 1,
            ']' | '}' | ')' => delta -= 1,
            _ => {}
        }
        previous = character;
    }
    delta
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kept_rules_are_the_plugin_s_rule_files() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../frontend-sdk/packages/eslint-plugin/rules");
        let mut shipped: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|entry| {
                let name = entry.unwrap().file_name().to_string_lossy().into_owned();
                name.strip_suffix(".cjs").map(str::to_string)
            })
            .collect();
        shipped.sort();
        assert_eq!(shipped, KEPT_RULES);
    }

    #[test]
    fn removes_dropped_rules_with_multi_line_values_and_keeps_the_rest() {
        let text = "    rules: {\n      \"skies/view-purity\": \"error\", //  // SKYFE001\n      \"skies/test-colocated\": \"error\", // SKYFE005\n      'skies/design-tokens': [\n        \"warn\",\n        { allow: [\"x\"] },\n      ],\n      \"skies/mutation-error-handled\": [\"error\", { globalSurface: true }], // SKYFE013\n      \"skies/feature-has-e2e-flow\": \"error\"\n    },\n";
        let mut plan = Plan::default();
        let out = migrate(text, "eslint.config.js", &mut plan).unwrap();
        assert_eq!(
            out,
            "    rules: {\n      \"skies/view-purity\": \"error\", //  // SKYFE001\n      \"skies/mutation-error-handled\": [\"error\", { globalSurface: true }], // SKYFE013\n    },\n"
        );
        assert!(migrate(&out, "eslint.config.js", &mut plan).is_none());
    }

    #[test]
    fn strips_the_react_native_rule_dropped_with_the_native_track() {
        let text = "      \"skies/data-door\": \"error\",\n      \"skies/viewmodel-platform-agnostic\": \"error\", // SKYFE009\n";
        let mut plan = Plan::default();
        let out = migrate(text, "eslint.config.mjs", &mut plan).unwrap();
        assert_eq!(out, "      \"skies/data-door\": \"error\",\n");
    }

    #[test]
    fn leaves_third_party_rules_such_as_the_a11y_floor_alone() {
        let text = "      \"jsx-a11y/alt-text\": \"off\",\n      \"jsx-a11y/aria-role\": [\"error\", { ignoreNonDOM: true }],\n      \"skies/no-mock\": \"error\",\n";
        assert!(migrate(text, "eslint.config.mjs", &mut Plan::default()).is_none());
    }

    #[test]
    fn json_configs_stay_valid() {
        let text = "{\n  \"rules\": {\n    \"skies/no-mock\": \"error\",\n    \"skies/ui-door\": \"error\"\n  }\n}\n";
        let out = migrate(text, ".eslintrc.json", &mut Plan::default()).unwrap();
        assert_eq!(out, "{\n  \"rules\": {\n    \"skies/no-mock\": \"error\"\n  }\n}\n");
    }

    #[test]
    fn recognizes_config_file_names() {
        assert!(is_config("eslint.config.mjs") && is_config(".eslintrc.cjs") && is_config(".eslintrc"));
        assert!(!is_config("eslint.config.test.js") && !is_config("eslint-rules.js"));
    }
}
