//! Test reports and the one rule of the proof system: failure modes and test cases must agree.
//!
//! The engine does not know xUnit, Playwright, or Flutter. A runner writes JUnit XML or a .NET TRX file, and a
//! case is tied to a failure mode only by its name (`"FM-2: double cancel"`, `FM2_double_cancel`). Nothing in
//! production code carries a tag.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::LazyLock;

use anyhow::{Context, Result, bail};
use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A failure-mode id such as `FM-3`. Ordered numerically so `FM-10` sorts after `FM-9` in receipts and output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FmId(pub u32);

impl fmt::Display for FmId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FM-{}", self.0)
    }
}

impl Serialize for FmId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for FmId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        match fm_ids(&text).as_slice() {
            [id] => Ok(*id),
            _ => Err(serde::de::Error::custom(format!("'{text}' is not a failure-mode id"))),
        }
    }
}

/// `FM` with an optional `-`, `_`, or space before the number, not glued to a preceding letter or digit, so
/// `PLATFORM-1` or `XFM2` never count while `Deposit_FM2`, `FM-2:` and `fm 2` do.
static FM_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?:^|[^a-z0-9])fm[-_ ]?(\d+)").expect("valid regex"));

/// Every failure-mode id a test name (or a spec line) mentions, deduplicated and in order.
pub fn fm_ids(text: &str) -> Vec<FmId> {
    let mut ids = BTreeSet::new();
    for capture in FM_PATTERN.captures_iter(text) {
        if let Ok(number) = capture[1].parse() {
            ids.insert(FmId(number));
        }
    }
    ids.into_iter().collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Passed,
    Failed,
    /// Skipped, not executed, or inconclusive. It never counts as passing: a skipped test proves nothing.
    Skipped,
}

#[derive(Debug, Clone)]
pub struct Case {
    pub name: String,
    pub outcome: Outcome,
    /// What a failed case reported (the assertion message, the start of the stack), when the report says.
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    JUnit,
    Trx,
}

impl Format {
    /// The file extension the evidence copy gets, so a reader's tooling opens it with the right viewer.
    pub fn extension(self) -> &'static str {
        match self {
            Format::JUnit => "xml",
            Format::Trx => "trx",
        }
    }
}

pub struct Report {
    pub format: Format,
    pub cases: Vec<Case>,
}

/// Parses a JUnit or TRX report, telling them apart by the root element rather than by file name, because
/// runners pick their own names.
pub fn parse(xml: &str) -> Result<Report> {
    let xml = xml.trim_start_matches('\u{feff}');
    let document = roxmltree::Document::parse(xml).context("the report is not well-formed XML")?;
    let root = document.root_element();
    match root.tag_name().name() {
        "testsuites" | "testsuite" => Ok(Report {
            format: Format::JUnit,
            cases: junit_cases(root),
        }),
        "TestRun" => Ok(Report {
            format: Format::Trx,
            cases: trx_cases(root),
        }),
        other => bail!("unrecognized report root <{other}>; expected JUnit <testsuites>/<testsuite> or TRX <TestRun>"),
    }
}

fn junit_cases(root: roxmltree::Node) -> Vec<Case> {
    root.descendants()
        .filter(|node| node.has_tag_name("testcase"))
        .map(|node| {
            let has = |tag: &str| node.children().any(|child| child.has_tag_name(tag));
            let outcome = if has("failure") || has("error") {
                Outcome::Failed
            } else if has("skipped") {
                Outcome::Skipped
            } else {
                Outcome::Passed
            };
            let message = node
                .children()
                .find(|child| child.has_tag_name("failure") || child.has_tag_name("error"))
                .and_then(|failure| {
                    let text = failure.text().map(str::trim).filter(|text| !text.is_empty());
                    text.or(failure.attribute("message")).map(String::from)
                });
            Case {
                name: node.attribute("name").unwrap_or_default().to_string(),
                outcome,
                message,
            }
        })
        .collect()
}

fn trx_cases(root: roxmltree::Node) -> Vec<Case> {
    // TRX elements live in the VisualStudio TeamTest namespace; matching on the local name keeps the parser
    // indifferent to which schema version wrote the file.
    root.descendants()
        .filter(|node| node.tag_name().name() == "UnitTestResult")
        .map(|node| {
            let outcome = match node.attribute("outcome").unwrap_or_default() {
                "Passed" => Outcome::Passed,
                "Failed" | "Error" | "Timeout" | "Aborted" => Outcome::Failed,
                _ => Outcome::Skipped,
            };
            let message = node
                .descendants()
                .find(|child| child.tag_name().name() == "ErrorInfo")
                .map(|info| {
                    info.children()
                        .filter(|part| matches!(part.tag_name().name(), "Message" | "StackTrace"))
                        .filter_map(|part| part.text().map(str::trim))
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .filter(|text| !text.is_empty());
            Case {
                name: node.attribute("testName").unwrap_or_default().to_string(),
                outcome,
                message,
            }
        })
        .collect()
}

/// What a run proved about each failure mode.
#[derive(Debug)]
pub struct Evaluation {
    /// Every failure mode in the spec: `true` when it has at least one case and all of them passed.
    pub passed: BTreeMap<FmId, bool>,
}

/// Why a run cannot be trusted as evidence at all, independent of pass or fail.
#[derive(Debug, Default)]
pub struct Inconsistency {
    pub uncovered: Vec<FmId>,
    pub unknown: Vec<(FmId, String)>,
}

impl Inconsistency {
    pub fn is_empty(&self) -> bool {
        self.uncovered.is_empty() && self.unknown.is_empty()
    }
}

impl fmt::Display for Inconsistency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut lines = Vec::new();
        for id in &self.uncovered {
            lines.push(format!(
                "{id} is listed in spec.md but no test case names it (title the case \"{id}: ...\")"
            ));
        }
        for (id, case) in &self.unknown {
            lines.push(format!(
                "case \"{case}\" names {id}, which spec.md does not list under ## Failure modes"
            ));
        }
        write!(f, "{}", lines.join("\n"))
    }
}

/// Maps each case to the failure modes it names, checks consistency, and decides pass/fail per failure mode.
pub fn evaluate(spec_fms: &[FmId], cases: &[Case]) -> Result<Evaluation, Inconsistency> {
    let listed: BTreeSet<FmId> = spec_fms.iter().copied().collect();
    let mut by_fm: BTreeMap<FmId, Vec<Outcome>> = BTreeMap::new();
    let mut problems = Inconsistency::default();
    for case in cases {
        for id in fm_ids(&case.name) {
            if listed.contains(&id) {
                by_fm.entry(id).or_default().push(case.outcome);
            } else {
                problems.unknown.push((id, case.name.clone()));
            }
        }
    }
    problems.uncovered = listed.iter().filter(|id| !by_fm.contains_key(id)).copied().collect();
    if !problems.is_empty() {
        return Err(problems);
    }
    let passed = by_fm
        .into_iter()
        .map(|(id, outcomes)| (id, outcomes.iter().all(|outcome| *outcome == Outcome::Passed)))
        .collect();
    Ok(Evaluation { passed })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_fm_ids_in_titles_and_method_names() {
        assert_eq!(fm_ids("FM-2: double cancel"), [FmId(2)]);
        assert_eq!(fm_ids("Specs.S0001.DepositSpec.FM2_double_cancel"), [FmId(2)]);
        assert_eq!(fm_ids("Deposit_fm_12_x"), [FmId(12)]);
        assert_eq!(fm_ids("fm 3 and FM-1"), [FmId(1), FmId(3)]);
        assert_eq!(fm_ids("FM-02"), [FmId(2)]);
    }

    #[test]
    fn ignores_fm_glued_to_other_words() {
        assert!(fm_ids("PLATFORM-1 boots").is_empty());
        assert!(fm_ids("XFM2").is_empty());
        assert!(fm_ids("2FM-1").is_empty());
        assert!(fm_ids("FM-: nothing").is_empty());
    }

    #[test]
    fn fm_ids_round_trip_through_json_in_numeric_order() {
        let map: BTreeMap<FmId, &str> = [(FmId(10), "pass"), (FmId(2), "fail")].into();
        let json = serde_json::to_string(&map).unwrap();
        assert_eq!(json, r#"{"FM-2":"fail","FM-10":"pass"}"#);
        let back: BTreeMap<FmId, String> = serde_json::from_str(&json).unwrap();
        assert_eq!(back.keys().copied().collect::<Vec<_>>(), [FmId(2), FmId(10)]);
    }

    #[test]
    fn parses_junit() {
        let report = parse(
            r#"<?xml version="1.0"?>
            <testsuites><testsuite name="s">
              <testcase name="FM-1: ok" classname="a"/>
              <testcase name="FM-2: broken"><failure message="boom"/></testcase>
              <testcase name="FM-3: crashed"><error/></testcase>
              <testcase name="FM-4: later"><skipped/></testcase>
            </testsuite></testsuites>"#,
        )
        .unwrap();
        assert_eq!(report.format, Format::JUnit);
        let outcomes: Vec<Outcome> = report.cases.iter().map(|case| case.outcome).collect();
        assert_eq!(
            outcomes,
            [Outcome::Passed, Outcome::Failed, Outcome::Failed, Outcome::Skipped]
        );
    }

    #[test]
    fn parses_a_bare_junit_testsuite() {
        let report = parse(r#"<testsuite><testcase name="FM-1: ok"/></testsuite>"#).unwrap();
        assert_eq!(report.cases.len(), 1);
    }

    #[test]
    fn parses_trx_with_bom_and_namespace() {
        let report = parse(
            "\u{feff}<?xml version=\"1.0\" encoding=\"utf-8\"?>
            <TestRun xmlns=\"http://microsoft.com/schemas/VisualStudio/TeamTest/2010\"><Results>
              <UnitTestResult testName=\"FM-1: a deposit\" outcome=\"Passed\"/>
              <UnitTestResult testName=\"FM-2: negative\" outcome=\"Failed\"/>
              <UnitTestResult testName=\"FM-3: later\" outcome=\"NotExecuted\"/>
            </Results></TestRun>",
        )
        .unwrap();
        assert_eq!(report.format, Format::Trx);
        let outcomes: Vec<Outcome> = report.cases.iter().map(|case| case.outcome).collect();
        assert_eq!(outcomes, [Outcome::Passed, Outcome::Failed, Outcome::Skipped]);
    }

    #[test]
    fn rejects_unknown_report_shapes() {
        let error = parse("<html/>").err().unwrap();
        assert!(error.to_string().contains("<html>"));
    }

    fn case(name: &str, outcome: Outcome) -> Case {
        Case {
            name: name.into(),
            outcome,
            message: None,
        }
    }

    #[test]
    fn keeps_what_a_failed_case_said() {
        let junit = parse(
            r#"<testsuite>
              <testcase name="FM-1: a"><failure message="expected 422">AssertionError: expected 422, got 200</failure></testcase>
              <testcase name="FM-2: b"><error message="boom"/></testcase>
              <testcase name="FM-3: c"/>
            </testsuite>"#,
        )
        .unwrap();
        let messages: Vec<Option<&str>> = junit.cases.iter().map(|case| case.message.as_deref()).collect();
        assert_eq!(
            messages,
            [Some("AssertionError: expected 422, got 200"), Some("boom"), None]
        );
        let trx = parse(
            "<TestRun><Results><UnitTestResult testName=\"FM-1: a\" outcome=\"Failed\"><Output><ErrorInfo>\
             <Message>Assert.Equal() Failure</Message><StackTrace>at Specs.S0001</StackTrace></ErrorInfo></Output>\
             </UnitTestResult></Results></TestRun>",
        )
        .unwrap();
        assert_eq!(
            trx.cases[0].message.as_deref(),
            Some("Assert.Equal() Failure\nat Specs.S0001")
        );
    }

    #[test]
    fn an_fm_passes_only_when_every_case_passes() {
        let cases = [
            case("FM-1: a", Outcome::Passed),
            case("FM-1: b", Outcome::Passed),
            case("FM-2: a", Outcome::Passed),
            case("FM-2: b", Outcome::Failed),
            case("FM-3: skipped", Outcome::Skipped),
            case("unrelated helper test", Outcome::Failed),
        ];
        let evaluation = evaluate(&[FmId(1), FmId(2), FmId(3)], &cases).unwrap();
        assert_eq!(
            evaluation.passed,
            [(FmId(1), true), (FmId(2), false), (FmId(3), false)].into()
        );
    }

    #[test]
    fn reports_uncovered_and_unknown_failure_modes() {
        let cases = [case("FM-1: a", Outcome::Passed), case("FM-9: stray", Outcome::Passed)];
        let problems = evaluate(&[FmId(1), FmId(2)], &cases).unwrap_err();
        assert_eq!(problems.uncovered, [FmId(2)]);
        assert_eq!(problems.unknown, [(FmId(9), "FM-9: stray".to_string())]);
        let message = problems.to_string();
        assert!(message.contains("FM-2 is listed in spec.md"));
        assert!(message.contains("names FM-9"));
    }
}
