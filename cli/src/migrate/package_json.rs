//! Edits a `package.json` line by line. npm writes one key per line and people review these files by diff, so the
//! migration changes only the lines it must and keeps every other byte, including key order and formatting. The
//! result is parsed back before it is used: an edit that would not yield valid JSON is reported, never written.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use regex::Regex;

use super::Plan;
use super::scripts::{self, Outcome};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Packages whose job ended with Skies 4: the gate tools and the Flutter tool bins.
const REMOVED: &[&str] = &["@skiesjs/frontend-sdk", "skies-frontend-sdk", "skies-flutter"];

/// Pre-scope names of the two packages Skies 5 still ships, with the scoped name each became in 4.1.6.
pub const RENAMED: &[(&str, &str)] = &[
    ("skies-react", "@skiesjs/react"),
    ("eslint-plugin-skies", "@skiesjs/eslint-plugin"),
];

const KEPT: &[&str] = &["@skiesjs/react", "@skiesjs/eslint-plugin"];

const DEPENDENCY_SECTIONS: &[&str] = &[
    "dependencies",
    "devDependencies",
    "peerDependencies",
    "optionalDependencies",
];

/// A `"key": "value"` entry on a line of its own.
static ENTRY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^(\s*)"((?:[^"\\]|\\.)*)"\s*:\s*("(?:[^"\\]|\\.)*")(,?)\s*$"#).unwrap());

/// A top-level `"section": {` opener.
static SECTION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"^\s*"([^"]+)"\s*:\s*\{\s*$"#).unwrap());

pub fn migrate(text: &str, relative: &str, plan: &mut Plan) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(text).ok()?;
    let declared = |name: &str| {
        DEPENDENCY_SECTIONS
            .iter()
            .any(|section| json.get(section).and_then(|deps| deps.get(name)).is_some())
    };
    let scripts: BTreeSet<String> = json
        .get("scripts")
        .and_then(|scripts| scripts.as_object())
        .map(|scripts| scripts.keys().cloned().collect())
        .unwrap_or_default();

    let mut lines: Vec<String> = text.split_inclusive('\n').map(str::to_string).collect();
    let mut notes: Vec<String> = Vec::new();
    let mut edited_scripts: Vec<String> = Vec::new();
    let mut dropped_scripts: BTreeSet<String> = BTreeSet::new();
    let mut section: Option<String> = None;
    let mut depth = 0i32;
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index].clone();
        if depth == 1
            && let Some(captures) = SECTION.captures(line.trim_end())
        {
            section = Some(captures[1].to_string());
        }
        let entry = (depth == 2)
            .then(|| ENTRY.captures(line.trim_end_matches(['\n', '\r'])))
            .flatten();
        depth += brace_delta(&line);
        if depth <= 1 {
            section = None;
        }
        let (Some(current), Some(entry)) = (section.as_deref(), entry) else {
            index += 1;
            continue;
        };
        let (indent, key, value, comma) = (&entry[1], &entry[2], &entry[3], &entry[4]);
        let value: String = serde_json::from_str(value).ok()?;
        let replacement = if DEPENDENCY_SECTIONS.contains(&current) {
            dependency(key, &value, &declared, &mut notes)
        } else if current == "scripts" {
            let mut script_notes = Vec::new();
            let outcome = scripts::rewrite(&value, &mut script_notes);
            notes.extend(script_notes.into_iter().map(|note| format!("script `{key}`: {note}")));
            match outcome {
                Outcome::Unchanged => None,
                Outcome::Dropped => {
                    dropped_scripts.insert(key.to_string());
                    Some(None)
                }
                Outcome::Rewritten(command) => {
                    edited_scripts.push(key.to_string());
                    Some(Some((key.to_string(), command)))
                }
            }
        } else {
            None
        };
        match replacement {
            None => index += 1,
            Some(None) => {
                lines.remove(index);
            }
            Some(Some((key, value))) => {
                lines[index] = format!("{indent}{}: {}{comma}\n", quote(&key), quote(&value));
                index += 1;
            }
        }
    }

    // A dropped script takes its npm lifecycle hooks and the `npm run` calls to it along.
    let mut hooks: BTreeSet<String> = BTreeSet::new();
    for name in &dropped_scripts {
        for hook in [format!("pre{name}"), format!("post{name}")] {
            if scripts.contains(&hook) {
                hooks.insert(hook);
            }
        }
    }
    dropped_scripts.extend(hooks.iter().cloned());
    if !dropped_scripts.is_empty() {
        drop_script_references(&mut lines, &dropped_scripts, &mut edited_scripts);
    }

    for note in notes {
        plan.follow_up_file(&note, relative);
    }
    if !edited_scripts.is_empty() || !dropped_scripts.is_empty() {
        let mut summary = Vec::new();
        if !edited_scripts.is_empty() {
            edited_scripts.sort();
            edited_scripts.dedup();
            summary.push(format!("rewrote {}", edited_scripts.join(", ")));
        }
        if !dropped_scripts.is_empty() {
            summary.push(format!(
                "removed {}",
                dropped_scripts.iter().cloned().collect::<Vec<_>>().join(", ")
            ));
        }
        plan.follow_up_file(&format!("scripts: {}; review them", summary.join("; ")), relative);
    }
    let edited = lines.concat();
    if edited == text {
        return None;
    }
    let out = fix_commas(&edited);
    if serde_json::from_str::<serde_json::Value>(&out).is_err() {
        plan.follow_up_file(
            "could not be edited safely; apply the Skies 5 dependency and script changes by hand",
            relative,
        );
        return None;
    }
    plan.follow_up("run `npm install` to refresh package-lock.json after the dependency changes");
    Some(out)
}

/// `None` keeps the line, `Some(None)` removes it, `Some(Some((key, value)))` rewrites it.
fn dependency(
    key: &str,
    value: &str,
    declared: &dyn Fn(&str) -> bool,
    notes: &mut Vec<String>,
) -> Option<Option<(String, String)>> {
    if REMOVED.contains(&key) || value.starts_with("npm:@skiesjs/frontend-sdk@") {
        return Some(None);
    }
    if let Some((_, scoped)) = RENAMED.iter().find(|(old, _)| *old == key) {
        if declared(scoped) {
            return Some(None);
        }
        notes.push(format!("`{key}` is now `{scoped}`; imports were renamed with it"));
        return Some(Some((scoped.to_string(), pinned(value))));
    }
    if KEPT.contains(&key) {
        let updated = match value.strip_prefix("npm:") {
            Some(alias) => {
                let (name, _) = alias.rsplit_once('@')?;
                format!("npm:{name}@{VERSION}")
            }
            None if value.starts_with(|c: char| c.is_ascii_digit() || c == '^' || c == '~') => pinned(value),
            None => return None,
        };
        return (updated != value).then(|| Some((key.to_string(), updated)));
    }
    if key == "avp-assay" || key == "assay-design" {
        notes.push(format!(
            "depends on `{key}`, an independent tool Skies 5 no longer ships; keep it or remove it"
        ));
    }
    None
}

/// The 5.x version, keeping the author's range operator.
fn pinned(value: &str) -> String {
    let operator: String = value.chars().take_while(|c| *c == '^' || *c == '~').collect();
    format!("{operator}{VERSION}")
}

fn quote(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

/// Removes the scripts in `dropped` and every `npm run <dropped>` segment of the remaining scripts.
fn drop_script_references(lines: &mut Vec<String>, dropped: &BTreeSet<String>, edited: &mut Vec<String>) {
    let mut depth = 0i32;
    let mut in_scripts = false;
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index].clone();
        if depth == 1 {
            in_scripts = SECTION
                .captures(line.trim_end())
                .is_some_and(|captures| &captures[1] == "scripts");
        }
        let entry = (depth == 2 && in_scripts)
            .then(|| ENTRY.captures(line.trim_end_matches(['\n', '\r'])))
            .flatten();
        depth += brace_delta(&line);
        let Some(entry) = entry else {
            index += 1;
            continue;
        };
        let key = entry[2].to_string();
        if dropped.contains(&key) {
            lines.remove(index);
            continue;
        }
        let Ok(command) = serde_json::from_str::<String>(&entry[3]) else {
            index += 1;
            continue;
        };
        let segments = scripts::split_and(&command);
        let kept: Vec<&str> = segments
            .iter()
            .copied()
            .filter(|segment| !scripts::npm_run_target(segment).is_some_and(|target| dropped.contains(target)))
            .collect();
        if kept.len() != segments.len() {
            edited.push(key.clone());
            if kept.is_empty() {
                lines.remove(index);
                continue;
            }
            lines[index] = format!(
                "{}{}: {}{}\n",
                &entry[1],
                quote(&key),
                quote(&kept.join(" && ")),
                &entry[4]
            );
        }
        index += 1;
    }
}

/// The change in object/array nesting a line makes, ignoring brackets inside strings.
fn brace_delta(line: &str) -> i32 {
    let (mut delta, mut in_string, mut escaped) = (0, false, false);
    for character in line.chars() {
        if in_string {
            match character {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match character {
            '"' => in_string = true,
            '{' | '[' => delta += 1,
            '}' | ']' => delta -= 1,
            _ => {}
        }
    }
    delta
}

/// Removing the last entry of an object leaves a trailing comma on the one before it, which JSON forbids; an object
/// left empty collapses to `{}` the way npm writes it.
pub fn fix_commas(text: &str) -> String {
    let mut lines: Vec<String> = text.split_inclusive('\n').map(str::to_string).collect();
    let mut index = 0;
    while index + 1 < lines.len() {
        let next = lines[index + 1].trim_start();
        let closes = next.starts_with('}') || next.starts_with(']');
        let body = lines[index].trim_end().to_string();
        if closes && (body.ends_with('{') || body.ends_with('[')) {
            let close = next.trim_end().to_string();
            let ending = &lines[index][body.len()..];
            lines[index] = format!("{body}{close}{ending}");
            lines.remove(index + 1);
            continue;
        }
        if closes && body.ends_with(',') {
            let ending = lines[index][body.len()..].to_string();
            lines[index] = format!("{}{ending}", &body[..body.len() - 1]);
        }
        index += 1;
    }
    lines.concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str) -> (Option<String>, Plan) {
        let mut plan = Plan::default();
        (migrate(text, "package.json", &mut plan), plan)
    }

    #[test]
    fn removes_and_bumps_dependencies_in_place() {
        let text = "{\n  \"name\": \"app\",\n  \"dependencies\": {\n    \"@skiesjs/react\": \"4.0.5\"\n  },\n  \"devDependencies\": {\n    \"@skiesjs/eslint-plugin\": \"^4.0.5\",\n    \"@skiesjs/frontend-sdk\": \"4.1.17\"\n  }\n}\n";
        let (out, _) = run(text);
        assert_eq!(
            out.unwrap(),
            format!(
                "{{\n  \"name\": \"app\",\n  \"dependencies\": {{\n    \"@skiesjs/react\": \"{VERSION}\"\n  }},\n  \"devDependencies\": {{\n    \"@skiesjs/eslint-plugin\": \"^{VERSION}\"\n  }}\n}}\n"
            )
        );
    }

    #[test]
    fn renames_pre_scope_packages_and_drops_aliases_of_the_removed_sdk() {
        let text = "{\n  \"dependencies\": {\n    \"react\": \"19.2.8\",\n    \"skies-react\": \"4.0.4\"\n  },\n  \"devDependencies\": {\n    \"@skiesjs/eslint-plugin\": \"4.0.5\",\n    \"eslint-plugin-skies\": \"4.0.4\",\n    \"skies-frontend-sdk\": \"npm:@skiesjs/frontend-sdk@4.1.14\",\n    \"sdk\": \"npm:@skiesjs/frontend-sdk@4.1.14\"\n  }\n}\n";
        let out = run(text).0.unwrap();
        assert!(
            out.contains(&format!("    \"@skiesjs/react\": \"{VERSION}\"\n  }},")),
            "{out}"
        );
        assert!(
            out.contains(&format!("\"@skiesjs/eslint-plugin\": \"{VERSION}\"\n  }}\n")),
            "{out}"
        );
        assert!(!out.contains("eslint-plugin-skies") && !out.contains("frontend-sdk"));
    }

    #[test]
    fn rewrites_scripts_and_removes_what_only_ran_the_gate() {
        let text = "{\n  \"scripts\": {\n    \"check\": \"npm run contract:check && npm run lint\",\n    \"contract:check\": \"skyfe-contract-freshness contract/Api.json src/client.gen\",\n    \"preverify:full\": \"node tools/stack.mjs reset\",\n    \"verify:full\": \"dotnet tool run skies check --task \\\"x\\\" --full\",\n    \"lint\": \"skies-flutter-doctor .\"\n  },\n  \"devDependencies\": {\n    \"skies-flutter\": \"4.1.24\"\n  }\n}\n";
        let (out, plan) = run(text);
        assert_eq!(
            out.unwrap(),
            "{\n  \"scripts\": {\n    \"check\": \"npm run lint\",\n    \"lint\": \"skies doctor --package .\"\n  },\n  \"devDependencies\": {}\n}\n"
        );
        let review = plan
            .follow_ups
            .keys()
            .find(|note| note.starts_with("scripts:"))
            .unwrap();
        assert_eq!(
            review,
            "scripts: rewrote check, lint; removed contract:check, preverify:full, verify:full; review them"
        );
    }

    #[test]
    fn a_migrated_manifest_is_left_alone() {
        let text = format!(
            "{{\n  \"scripts\": {{\n    \"lint\": \"skies doctor --package .\"\n  }},\n  \"dependencies\": {{\n    \"@skiesjs/react\": \"{VERSION}\"\n  }}\n}}\n"
        );
        assert!(run(&text).0.is_none());
    }
}
