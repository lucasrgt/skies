//! The SKYFL escape hatch: `// skies-ignore: SKYFL029 <reason>` on a finding's line or the line above it, or
//! `// skies-ignore-file: SKYFL001 <reason>` anywhere in the file for a finding with no line (a missing sibling, a
//! file-wide state gap). It is the Flutter spelling of `#pragma warning disable` / `[SuppressMessage]` on .NET and
//! `eslint-disable-next-line` on the web, with two constraints the others lack by default:
//!
//! - **One rule per directive, and a reason.** A directive without a reason suppresses nothing; the finding stays
//!   and says why, so an escape hatch cannot be taken silently.
//! - **Visible.** A suppressed finding is not dropped: the doctor prints it, with its reason, under its leg.
//!
//! A rule that keeps needing the hatch is a finding against the rule, to report rather than to paper over.

use std::collections::HashMap;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use super::Source;
use super::facts::Located;
use crate::doctor::{Finding, Suppressed};

/// One `skies-ignore` directive in a comment.
#[derive(Debug, PartialEq)]
struct Directive {
    code: String,
    /// `None` when the directive names no reason, which makes it inert.
    reason: Option<String>,
    line: usize,
    file: bool,
}

static DIRECTIVE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"skies-ignore(?P<file>-file)?:\s*(?P<code>SKYFL[0-9]{3})\b(?P<reason>.*)").expect("static pattern")
});

fn directives(comments: &[Located<String>]) -> Vec<Directive> {
    comments
        .iter()
        .filter_map(|comment| {
            let captures = DIRECTIVE.captures(&comment.value)?;
            let reason = captures["reason"]
                .trim()
                .trim_end_matches("*/")
                .trim_start_matches(['-', ':', '—'])
                .trim();
            Some(Directive {
                code: captures["code"].to_string(),
                reason: (!reason.is_empty()).then(|| reason.to_string()),
                line: comment.line,
                file: captures.name("file").is_some(),
            })
        })
        .collect()
}

/// Whether a directive covers a finding: the same rule, and the finding's line (or the line below a directive
/// written above it), or any line for a file-wide directive.
fn covers(directive: &Directive, finding: &Finding) -> bool {
    directive.code == finding.code
        && (directive.file
            || finding
                .line
                .is_some_and(|line| line == directive.line || line == directive.line + 1))
}

/// Splits findings into those that stand and those a reasoned directive suppresses. A covering directive without
/// a reason leaves the finding standing and appends why to its message.
pub fn apply(findings: Vec<Finding>, sources: &[Source]) -> (Vec<Finding>, Vec<Suppressed>) {
    let by_path: HashMap<&Path, Vec<Directive>> = sources
        .iter()
        .map(|source| (source.path.as_path(), directives(&source.facts.comments)))
        .filter(|(_, found)| !found.is_empty())
        .collect();
    let mut standing = Vec::new();
    let mut suppressed = Vec::new();
    for mut finding in findings {
        let directive = by_path
            .get(finding.file.as_path())
            .and_then(|found| found.iter().find(|d| covers(d, &finding)));
        match directive {
            Some(Directive {
                reason: Some(reason), ..
            }) => suppressed.push(Suppressed {
                finding,
                reason: reason.clone(),
            }),
            Some(Directive { line, .. }) => {
                finding.message = format!(
                    "{} (the skies-ignore on line {line} names no reason, so it does not suppress)",
                    finding.message
                );
                standing.push(finding);
            }
            None => standing.push(finding),
        }
    }
    (standing, suppressed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn comment(line: usize, value: &str) -> Located<String> {
        Located {
            value: value.to_string(),
            line,
        }
    }

    #[test]
    fn a_directive_names_one_rule_and_its_reason() {
        let found = directives(&[
            comment(
                3,
                "// skies-ignore: SKYFL029 the legacy client rotates on its own until the port",
            ),
            comment(9, "// skies-ignore-file: SKYFL001 -- a platform view with no ViewModel"),
            comment(12, "/* skies-ignore: SKYFL016 */"),
            comment(14, "// skies-ignore: SKY0006 not a Flutter rule"),
            comment(15, "// a plain comment"),
        ]);
        assert_eq!(
            found,
            [
                Directive {
                    code: "SKYFL029".into(),
                    reason: Some("the legacy client rotates on its own until the port".into()),
                    line: 3,
                    file: false,
                },
                Directive {
                    code: "SKYFL001".into(),
                    reason: Some("a platform view with no ViewModel".into()),
                    line: 9,
                    file: true,
                },
                Directive {
                    code: "SKYFL016".into(),
                    reason: None,
                    line: 12,
                    file: false,
                },
            ]
        );
    }
}
