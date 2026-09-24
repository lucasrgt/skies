//! The agent plugin carries the convention docs an agent loads, and they must never drift from the docs the
//! framework maintains. So `skies-plugin/docs/` holds verbatim copies of the convention docs plus a CLI reference
//! rendered from `skies --help`, and these tests fail whenever either falls behind its source.
//!
//! Resync with `tools/sync-plugin-docs.sh`, which reruns these tests with `SKIES_SYNC_PLUGIN_DOCS=1`: in that mode
//! they write the expected content instead of comparing it.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The docs copied into the plugin, by file name, from `docs/`. Relative links between them keep working because
/// the copies keep the same names side by side.
const COPIED: [&str; 3] = ["CONVENTIONS.md", "FRONTEND-CONVENTIONS.md", "FLUTTER-CONVENTIONS.md"];

const CLI_REFERENCE: &str = "cli.md";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn syncing() -> bool {
    std::env::var_os("SKIES_SYNC_PLUGIN_DOCS").is_some_and(|value| value == "1")
}

/// Compares `path` with `expected`, or writes it when syncing. Returns a line naming the drift, if any.
fn check(path: &Path, expected: &str) -> Option<String> {
    if syncing() {
        std::fs::write(path, expected).unwrap();
        return None;
    }
    let actual = std::fs::read_to_string(path).unwrap_or_default();
    (actual != expected).then(|| format!("  {} differs from its source", path.display()))
}

fn fail_on(drift: Vec<String>) {
    assert!(
        drift.is_empty(),
        "the plugin docs drifted:\n{}\nresync with tools/sync-plugin-docs.sh",
        drift.join("\n")
    );
}

#[test]
fn the_plugin_carries_the_convention_docs_verbatim() {
    let root = root();
    let drift = COPIED
        .iter()
        .filter_map(|name| {
            let source = std::fs::read_to_string(root.join("docs").join(name)).unwrap();
            check(&root.join("skies-plugin/docs").join(name), &source)
        })
        .collect();
    fail_on(drift);
}

#[test]
fn the_plugin_cli_reference_is_the_help_text() {
    let path = root().join("skies-plugin/docs").join(CLI_REFERENCE);
    fail_on(check(&path, &cli_reference()).into_iter().collect());
}

/// Every command's `--help`, depth first, under one heading each.
fn cli_reference() -> String {
    let mut text = String::from(
        "# Skies — CLI reference\n\n\
         Generated from `skies <command> --help` by `cli/tests/plugin_docs.rs`; resync with\n\
         `tools/sync-plugin-docs.sh` instead of editing.\n",
    );
    let mut pending = vec![Vec::<String>::new()];
    while let Some(path) = pending.pop() {
        let help = help(&path);
        text.push_str(&format!(
            "\n## skies{}\n\n```text\n{help}\n```\n",
            path.iter().map(|word| format!(" {word}")).collect::<String>()
        ));
        let mut children: Vec<Vec<String>> = subcommands(&help)
            .into_iter()
            .map(|name| path.iter().cloned().chain([name]).collect())
            .collect();
        children.reverse();
        pending.extend(children);
    }
    text
}

fn help(path: &[String]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_skies"))
        .args(path)
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success(), "skies {} --help failed", path.join(" "));
    let text = String::from_utf8(output.stdout).unwrap();
    text.lines().map(str::trim_end).collect::<Vec<_>>().join("\n").trim().to_string()
}

/// The names listed under `Commands:`, minus clap's own `help`.
fn subcommands(help: &str) -> Vec<String> {
    help.lines()
        .skip_while(|line| *line != "Commands:")
        .skip(1)
        .take_while(|line| line.starts_with("  "))
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| *name != "help")
        .map(String::from)
        .collect()
}
