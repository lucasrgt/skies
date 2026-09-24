//! Rewrites one `package.json` script command. Skies 4 packages wired their gate into npm scripts through bins that
//! no longer exist (`skyfe-*`, the `skies-flutter-*` checkers, `dotnet tool run skies check`); a script that still
//! calls one fails the moment the 4.x packages leave `node_modules`. Each `&&` segment is judged on its own: gate
//! segments go, the three 4.x commands with a 5.x equivalent are rewritten, and everything else stays as written.

/// What happened to a whole script.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    Unchanged,
    Rewritten(String),
    /// Every segment ran a removed tool; the script itself goes.
    Dropped,
}

/// Flutter SDK bins whose job moved into `skies doctor` or disappeared with the gate.
const REMOVED_FLUTTER_BINS: &[&str] = &[
    "skies-flutter-e2e-doctor",
    "skies-flutter-feature-e2e",
    "skies-flutter-journey-parity",
    "skies-flutter-framework-sync",
    "skies-flutter-parity",
    "skies-flutter-endpoint-coverage",
    "skies-flutter-contract-freshness",
    "skies-flutter-design",
];

/// Skies CLI subcommands that were the gate and have no 5.x counterpart.
const GATE_SUBCOMMANDS: &[&str] = &["check", "gate", "context"];

/// Flags the 4.x Flutter doctor took that the 5.x doctor has no use for: e2e checks are gone, so
/// `--structure-only` is what every run now does.
const OBSOLETE_DOCTOR_FLAGS: &[&str] = &["--structure-only"];

/// Rewrites `command`, adding a note for every judgement a person should review.
pub fn rewrite(command: &str, notes: &mut Vec<String>) -> Outcome {
    let segments = split_and(command);
    let mut kept = Vec::with_capacity(segments.len());
    let mut changed = false;
    for segment in &segments {
        match rewrite_segment(segment, notes) {
            Some(Some(replacement)) => {
                changed |= replacement != *segment;
                kept.push(replacement);
            }
            Some(None) => changed = true,
            None => kept.push(segment.to_string()),
        }
    }
    if !changed {
        Outcome::Unchanged
    } else if kept.is_empty() {
        Outcome::Dropped
    } else {
        Outcome::Rewritten(kept.join(" && "))
    }
}

/// `None` keeps the segment untouched; `Some(None)` drops it; `Some(Some(text))` replaces it.
fn rewrite_segment(segment: &str, notes: &mut Vec<String>) -> Option<Option<String>> {
    let tokens: Vec<&str> = segment.split_whitespace().collect();
    let start = program_index(&tokens)?;
    let (program, args) = (tokens[start], &tokens[start + 1..]);
    match program {
        bin if bin.starts_with("skyfe-") || REMOVED_FLUTTER_BINS.contains(&bin) => Some(None),
        "skies" if args.first().is_some_and(|sub| GATE_SUBCOMMANDS.contains(sub)) => Some(None),
        "dotnet" => dotnet_tool(args, notes),
        "skies-flutter-doctor" => Some(Some(flutter_doctor(args, notes))),
        "skies-flutter-client" => Some(Some(flutter_client(args, notes))),
        "skies-flutter-i18n" => {
            notes.push("`skies-flutter-i18n` became `skies i18n`, which assembles the package's catalogs".into());
            Some(Some("skies i18n".into()))
        }
        "skies-flutter-feature" | "skies-flutter-app" | "skies-flutter-client-scaffold" => {
            notes.push(format!(
                "`{program}` is gone; use `skies g feature`, `skies g flutter-app`, or `skies g client`"
            ));
            None
        }
        "assay" | "avp-assay" | "assay-design" => {
            notes.push(format!(
                "runs `{program}`, an independent tool Skies 5 no longer ships; keep it or remove it"
            ));
            None
        }
        _ => None,
    }
}

/// The index of the program a segment runs, past environment assignments and an `npx` launcher.
fn program_index(tokens: &[&str]) -> Option<usize> {
    let mut index = 0;
    while tokens.get(index).is_some_and(|token| is_assignment(token)) {
        index += 1;
    }
    if tokens.get(index) == Some(&"npx") {
        index += 1;
        while tokens.get(index).is_some_and(|token| token.starts_with('-')) {
            index += 1;
        }
    }
    (index < tokens.len()).then_some(index)
}

fn is_assignment(token: &str) -> bool {
    token.split_once('=').is_some_and(|(name, _)| {
        !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
    })
}

/// `dotnet tool run skies check …` is the gate; any other `dotnet tool run skies …` is the same command on the
/// 5.x binary, which is no longer a dotnet tool.
fn dotnet_tool(args: &[&str], notes: &mut Vec<String>) -> Option<Option<String>> {
    let rest = match args {
        ["tool", "run", "skies", rest @ ..] | ["skies", rest @ ..] => rest,
        _ => return None,
    };
    if rest.first().is_some_and(|sub| GATE_SUBCOMMANDS.contains(sub)) {
        return Some(None);
    }
    notes.push("the `skies` dotnet tool is now the `skies` binary on PATH".into());
    Some(Some(
        std::iter::once("skies")
            .chain(rest.iter().copied())
            .collect::<Vec<_>>()
            .join(" "),
    ))
}

/// `skies-flutter-doctor <dir> [--flags]` → `skies doctor --package <dir>`.
fn flutter_doctor(args: &[&str], notes: &mut Vec<String>) -> String {
    let mut dir = None;
    let mut dropped = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let arg = args[index];
        if arg.starts_with("--") {
            let takes_value = ["--contract", "--backend-root"].contains(&arg);
            if !OBSOLETE_DOCTOR_FLAGS.contains(&arg) {
                dropped.push(if takes_value {
                    format!("{arg} {}", args.get(index + 1).unwrap_or(&""))
                } else {
                    arg.to_string()
                });
            }
            index += if takes_value { 2 } else { 1 };
            continue;
        }
        dir.get_or_insert(arg);
        index += 1;
    }
    if !dropped.is_empty() {
        notes.push(format!(
            "`skies-flutter-doctor` flag(s) {} have no `skies doctor` equivalent and were dropped",
            dropped.join(", ")
        ));
    }
    format!("skies doctor --package {}", dir.unwrap_or("."))
}

/// `skies-flutter-client --input … --output … --name … --version …` → `skies g client --package . …`; the flag names
/// are the same, and relative paths still resolve against the script's directory.
fn flutter_client(args: &[&str], notes: &mut Vec<String>) -> String {
    let known = ["--input", "--output", "--name", "--version"];
    let unknown: Vec<&str> = args
        .iter()
        .copied()
        .filter(|arg| arg.starts_with("--") && !known.contains(&arg.split('=').next().unwrap_or(arg)))
        .collect();
    if !unknown.is_empty() {
        notes.push(format!("`skies g client` does not take {}", unknown.join(", ")));
    }
    std::iter::once("skies g client --package .")
        .chain(args.iter().copied())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Splits a shell command on `&&` outside quotes, trimming each segment.
pub fn split_and(command: &str) -> Vec<&str> {
    let bytes = command.as_bytes();
    let (mut quote, mut start, mut index) = (None::<u8>, 0, 0);
    let mut segments = Vec::new();
    while index < bytes.len() {
        let byte = bytes[index];
        match quote {
            Some(open) if byte == open => quote = None,
            Some(_) if byte == b'\\' => index += 1,
            Some(_) => {}
            None if byte == b'"' || byte == b'\'' => quote = Some(byte),
            None if byte == b'&' && bytes.get(index + 1) == Some(&b'&') => {
                segments.push(command[start..index].trim());
                index += 1;
                start = index + 1;
            }
            None => {}
        }
        index += 1;
    }
    segments.push(command[start..].trim());
    segments.into_iter().filter(|segment| !segment.is_empty()).collect()
}

/// The script an `npm run <name>` segment calls, if that is all the segment does.
pub fn npm_run_target(segment: &str) -> Option<&str> {
    let tokens: Vec<&str> = segment.split_whitespace().collect();
    let rest = match tokens.as_slice() {
        ["npm", "run" | "run-script", rest @ ..] => rest,
        _ => return None,
    };
    let mut names = rest.iter().filter(|token| !token.starts_with('-'));
    let name = names.next()?;
    names.next().is_none().then_some(*name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(command: &str) -> (Outcome, Vec<String>) {
        let mut notes = Vec::new();
        (rewrite(command, &mut notes), notes)
    }

    #[test]
    fn drops_gate_segments_and_keeps_the_rest() {
        let (outcome, _) = run(
            "eslint . && npm run lint:native-overlays && skyfe-framework-sync ../.. && skyfe-e2e-doctor . && skies-flutter-parity x",
        );
        assert_eq!(
            outcome,
            Outcome::Rewritten("eslint . && npm run lint:native-overlays".into())
        );
        assert_eq!(
            run("npm run spec:fetch && orval && skyfe-contract-freshness contract/Api.json src/client.gen --stamp").0,
            Outcome::Rewritten("npm run spec:fetch && orval".into())
        );
    }

    #[test]
    fn a_script_made_only_of_gate_segments_is_dropped() {
        assert_eq!(
            run("dotnet tool run skies check --task \"local release verification\" --full").0,
            Outcome::Dropped
        );
        assert_eq!(run("skyfe-e2e-doctor .").0, Outcome::Dropped);
        assert_eq!(run("npx --yes skies gate").0, Outcome::Dropped);
    }

    #[test]
    fn maps_the_flutter_commands_onto_the_binary() {
        assert_eq!(
            run("skies-flutter-doctor . --structure-only"),
            (Outcome::Rewritten("skies doctor --package .".into()), vec![])
        );
        let (outcome, notes) = run("skies-flutter-doctor app --strict");
        assert_eq!(outcome, Outcome::Rewritten("skies doctor --package app".into()));
        assert!(notes[0].contains("--strict"));
        assert_eq!(
            run(
                "npm run generate:contract && skies-flutter-client --input contract/H.Api.json --output packages/h_api --name h_api --version 1.0.0"
            )
            .0,
            Outcome::Rewritten(
                "npm run generate:contract && skies g client --package . --input contract/H.Api.json --output packages/h_api --name h_api --version 1.0.0"
                    .into()
            )
        );
        assert_eq!(
            run("skies-flutter-i18n assemble lib").0,
            Outcome::Rewritten("skies i18n".into())
        );
    }

    #[test]
    fn leaves_application_commands_and_reports_independent_tools() {
        assert_eq!(run("skies doctor").0, Outcome::Unchanged);
        assert_eq!(run("node -e \"a && skyfe-x\"").0, Outcome::Unchanged);
        let (outcome, notes) = run("assay-design doctor --contract .design/contract.toml");
        assert_eq!(outcome, Outcome::Unchanged);
        assert_eq!(notes.len(), 1);
    }

    #[test]
    fn finds_the_script_an_npm_run_segment_calls() {
        assert_eq!(npm_run_target("npm run contract:check"), Some("contract:check"));
        assert_eq!(npm_run_target("npm run --silent lint"), Some("lint"));
        assert_eq!(npm_run_target("npm --workspace x run check"), None);
    }
}
