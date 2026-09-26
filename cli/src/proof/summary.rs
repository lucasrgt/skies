//! What a receipt keeps from a report: per failure mode, the cases that decided it and, on red, the start of what
//! the first failing case said; and when red did not build, the lines of output that say why.
//!
//! The full report stays local (evidence/raw/), so this is what a reviewer reads in the pull request. It must be
//! short, and stable: the same run twice yields the same text (no timings, checkout paths replaced, cases sorted).

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use super::report::{Case, Outcome};

/// The checkouts a run's messages may name (the working tree, a temporary red worktree), replaced by `{root}` so a
/// receipt reads the same on every machine and every run.
pub struct Scrub(Vec<String>);

impl Scrub {
    pub fn new(roots: &[&Path]) -> Scrub {
        let mut roots: Vec<String> = roots
            .iter()
            .map(|root| root.to_string_lossy().into_owned())
            .filter(|root| !root.is_empty())
            .collect();
        // The longest first, so a worktree inside another root is replaced whole.
        roots.sort_by_key(|root| std::cmp::Reverse(root.len()));
        Scrub(roots)
    }

    pub fn text(&self, text: &str) -> String {
        self.0
            .iter()
            .fold(text.to_string(), |text, root| text.replace(root.as_str(), "{root}"))
    }
}

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

/// Why red did not build, from the lines that tie it to the spec's e2e: the first few, checkout paths replaced.
pub fn excerpt(lines: &[String], scrub: &Scrub) -> Option<String> {
    let kept: Vec<String> = lines
        .iter()
        .take(LINES)
        .map(|line| clip(&scrub.text(&ANSI.replace_all(line, ""))))
        .collect();
    (!kept.is_empty()).then(|| kept.join("\n"))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn case(name: &str, outcome: Outcome, message: Option<&str>) -> Case {
        Case {
            name: name.into(),
            outcome,
            message: message.map(String::from),
            file: None,
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
        let scrub = Scrub::new(&[Path::new("/tmp/w")]);
        assert_eq!(first_failure(&cases, &scrub).as_deref(), Some("first"));
        assert_eq!(scrub.text("at /tmp/w/src/X.cs"), "at {root}/src/X.cs");
        assert_eq!(first_failure(&cases[2..], &scrub), None);
    }

    #[test]
    fn did_not_build_keeps_the_first_lines_without_checkout_paths() {
        let lines: Vec<String> = (1..=6)
            .map(|n| format!("/tmp/w/.specs/0001-a/e2e/X.cs({n},5): error CS0246: 'Transfer' could not be found"))
            .collect();
        let text = excerpt(&lines, &Scrub::new(&[Path::new("/tmp/w")])).unwrap();
        assert_eq!(text.lines().count(), 4);
        assert!(
            text.starts_with("{root}/.specs/0001-a/e2e/X.cs(1,5): error CS0246"),
            "{text}"
        );
        assert_eq!(excerpt(&[], &Scrub::new(&[])), None);
    }
}
