//! What a receipt keeps from a report: per failure mode, the cases that decided it and, on red, the start of what
//! the first failing case said; and when red did not build, the lines of output that say why.
//!
//! The full report stays local (evidence/raw/), so this is what a reviewer reads in the pull request. It must be
//! short, and stable: the same run twice yields the same text (no timings, machine paths scrubbed, cases sorted).

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use super::report::{Case, Outcome};
use super::scrub::Scrub;

/// At most this many lines of a message, each cut to [`WIDTH`] characters.
const LINES: usize = 4;
const WIDTH: usize = 200;

/// The names of the cases, sorted and deduplicated, so the order a runner happened to use never shows in a diff.
pub fn names(cases: &[Case]) -> Vec<String> {
    let mut names: Vec<String> = cases.iter().map(|case| case.name.clone()).collect();
    names.sort();
    names.dedup();
    names
}

/// What the first failing case (by name) reported, trimmed: the assertion, not the stack.
pub fn first_failure(cases: &[Case], scrub: &Scrub) -> Option<String> {
    let mut failing: Vec<&Case> = cases.iter().filter(|case| case.outcome != Outcome::Passed).collect();
    failing.sort_by(|a, b| a.name.cmp(&b.name));
    failing
        .iter()
        .find_map(|case| case.message.as_deref())
        .map(|message| trim(&scrub.text(message)))
        .filter(|message| !message.is_empty())
}

/// The first few non-blank lines of a failure message, stopping where the stack trace starts.
pub fn trim(text: &str) -> String {
    let text = ANSI.replace_all(text, "");
    let mut kept = Vec::new();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if is_frame(line) || kept.len() == LINES {
            break;
        }
        kept.push(clip(line));
    }
    kept.join("\n")
}

/// Why red did not build, from the runner's output: its error lines (each once), else its last lines.
pub fn output(log: &Path, scrub: &Scrub) -> Option<String> {
    let text = std::fs::read_to_string(log).ok()?;
    let text = ANSI.replace_all(&text, "");
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|line| !line.is_empty()).collect();
    let mut errors: Vec<&str> = Vec::new();
    for line in lines.iter().filter(|line| ERROR.is_match(line)) {
        if !errors.contains(line) {
            errors.push(line);
        }
    }
    let chosen: Vec<&str> = if errors.is_empty() {
        lines[lines.len().saturating_sub(LINES)..].to_vec()
    } else {
        errors.into_iter().take(LINES).collect()
    };
    let excerpt: Vec<String> = chosen.iter().map(|line| clip(&scrub.text(line))).collect();
    (!excerpt.is_empty()).then(|| excerpt.join("\n"))
}

/// A stack frame in .NET (`at Specs.S0001…`), vitest (`❯ file:line`), or Dart (`#0 …`).
fn is_frame(line: &str) -> bool {
    line.starts_with("at ") || line.starts_with('❯') || line.starts_with("--- End of") || FRAME.is_match(line)
}

fn clip(line: &str) -> String {
    if line.chars().count() <= WIDTH {
        return line.to_string();
    }
    let cut: String = line.chars().take(WIDTH - 1).collect();
    format!("{}…", cut.trim_end())
}

static ANSI: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\x1b\[[0-9;]*[A-Za-z]").expect("valid regex"));

static FRAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^#\d+\s").expect("valid regex"));

/// A compiler or runner error line: `error CS0246`, `Error: Cannot find module`, `SyntaxError: …`, `MISSING DEPENDENCY`.
static ERROR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(\berror\b[ :A-Z0-9]|\w+Error:|MISSING DEPENDENCY|cannot find)").expect("valid regex")
});

#[cfg(test)]
mod tests {
    use super::*;

    fn case(name: &str, outcome: Outcome, message: Option<&str>) -> Case {
        Case {
            name: name.into(),
            outcome,
            message: message.map(String::from),
        }
    }

    #[test]
    fn keeps_the_assertion_and_drops_the_stack() {
        let trx = "Assert.Equal() Failure: Values differ\nExpected: Created\nActual:   NotFound\n\
                   at Specs.S0001.DepositSpec.FM1() in /repo/.specs/0001/e2e/DepositSpec.cs:line 30";
        assert_eq!(
            trim(trx),
            "Assert.Equal() Failure: Values differ\nExpected: Created\nActual:   NotFound"
        );
        let vitest = "\u{1b}[31mAssertionError: expected 'x' to be 'y'\u{1b}[39m\n ❯ Deposit.test.tsx:12:5";
        assert_eq!(trim(vitest), "AssertionError: expected 'x' to be 'y'");
        let long = "a\nb\nc\nd\ne\nf";
        assert_eq!(trim(long), "a\nb\nc\nd");
        assert!(trim(&"x".repeat(500)).ends_with('…'));
    }

    #[test]
    fn names_are_sorted_and_the_first_failure_is_stable() {
        let cases = [
            case("FM-1: b", Outcome::Failed, Some("second")),
            case("FM-1: a", Outcome::Failed, Some("first\nat frame")),
            case("FM-1: c", Outcome::Passed, None),
        ];
        assert_eq!(names(&cases), ["FM-1: a", "FM-1: b", "FM-1: c"]);
        let scrub = Scrub::new(&[]);
        assert_eq!(first_failure(&cases, &scrub).as_deref(), Some("first"));
        assert_eq!(first_failure(&cases[2..], &scrub), None);
    }

    #[test]
    fn did_not_build_keeps_each_error_line_once() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("red.log");
        std::fs::write(
            &log,
            "Build started\nX.cs(3,5): error CS0246: The type 'Transfer' could not be found\n\
             Build FAILED.\nX.cs(3,5): error CS0246: The type 'Transfer' could not be found\nTime Elapsed 00:00:02.13\n",
        )
        .unwrap();
        assert_eq!(
            output(&log, &Scrub::new(&[])).as_deref(),
            Some("X.cs(3,5): error CS0246: The type 'Transfer' could not be found")
        );
        std::fs::write(&log, "one\ntwo\n").unwrap();
        assert_eq!(output(&log, &Scrub::new(&[])).as_deref(), Some("one\ntwo"));
    }
}
