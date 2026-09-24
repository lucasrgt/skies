//! Portable evidence: what `record` and `verify` rewrite in a test report or build log as it enters evidence/.
//!
//! Evidence is committed, so it must not carry the machine it ran on. A TRX or JUnit report names the host and
//! the user, and its output and stack traces are full of absolute paths into the checkout, the red worktree under
//! the temp directory, and the home directory. Those become `{root}`, `{tmp}`, and `~`; host and run names become
//! `{machine}` and `{run}`; and a TRX's per-run GUIDs are renumbered in order of appearance, so two recordings of
//! the same run differ only where the run did. A TRX's data-collector attachments (coverage) are dropped, since
//! the files they link to stay out of evidence. Results, test names, durations, and messages are left as written.
//! The placeholders use braces, not angle brackets, so a rewritten report stays well-formed XML.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use anyhow::{Context, Result};
use regex::Regex;

use super::report::Format;

/// Rewrites machine-specific text for one run. Built once per run from the checkouts it used.
pub struct Scrub {
    /// Absolute paths to replace, longest first so a checkout under the temp directory wins over `{tmp}`.
    paths: Vec<(Regex, &'static str)>,
}

impl Scrub {
    /// `checkouts` are the top levels of the checkouts the run used: the working tree and, for `record`, the red
    /// worktree. Call it while they exist, so their canonical spellings can be resolved too.
    pub fn new(checkouts: &[&Path]) -> Scrub {
        let mut roots: Vec<(PathBuf, &'static str)> = Vec::new();
        for checkout in checkouts {
            roots.push((checkout.to_path_buf(), "{root}"));
        }
        roots.push((std::env::temp_dir(), "{tmp}"));
        if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
            roots.push((PathBuf::from(home), "~"));
        }
        let mut spelled: Vec<(String, &'static str)> = Vec::new();
        for (path, placeholder) in roots {
            let canonical = std::fs::canonicalize(&path).ok();
            for variant in std::iter::once(path).chain(canonical) {
                let text = variant.to_string_lossy().trim_end_matches(['/', '\\']).to_string();
                // A root of `/` or a bare drive would rewrite every absolute path in the file.
                if text.len() > 3 && !spelled.iter().any(|(known, _)| known.eq_ignore_ascii_case(&text)) {
                    spelled.push((text, placeholder));
                }
            }
        }
        spelled.sort_by_key(|(text, _)| std::cmp::Reverse(text.len()));
        let paths = spelled
            .into_iter()
            .map(|(text, placeholder)| (path_pattern(&text), placeholder))
            .collect();
        Scrub { paths }
    }

    /// Absolute paths only: for build logs and anything else that is not a report.
    pub fn text(&self, text: &str) -> String {
        self.paths
            .iter()
            .fold(text.to_string(), |text, (pattern, placeholder)| {
                replace_bounded(&text, pattern, placeholder)
            })
    }

    /// Paths, host and user names, and (for TRX) run ids. Test names are never rewritten, even where one happens
    /// to spell a path.
    pub fn report(&self, text: &str, format: Format) -> String {
        let mut scrubbed = String::with_capacity(text.len());
        let mut last = 0;
        for name in TEST_NAMES.find_iter(text) {
            scrubbed.push_str(&self.text(&text[last..name.start()]));
            scrubbed.push_str(name.as_str());
            last = name.end();
        }
        scrubbed.push_str(&self.text(&text[last..]));
        let text = scrubbed;
        let text = HOST.replace_all(&text, "${1}=\"{machine}\"");
        match format {
            Format::JUnit => text.into_owned(),
            Format::Trx => {
                let text = RUN_NAME.replace_all(&text, "${1}{run}${2}");
                let text = DEPLOYMENT.replace_all(&text, "${1}=\"{run}\"");
                let text = COLLECTORS.replace_all(&text, "");
                renumber_run_ids(&text)
            }
        }
    }

    /// Copies `source` to `target`, rewriting reports (`.trx`, `.xml`) and logs (`.log`) and copying anything else
    /// byte for byte.
    pub fn copy(&self, source: &Path, target: &Path) -> Result<()> {
        let extension = target
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default();
        let format = match extension {
            "trx" => Some(Format::Trx),
            "xml" => Some(Format::JUnit),
            "log" => None,
            _ => {
                std::fs::copy(source, target).with_context(|| format!("copying {}", source.display()))?;
                return Ok(());
            }
        };
        let text = std::fs::read_to_string(source).with_context(|| format!("reading {}", source.display()))?;
        let scrubbed = match format {
            Some(format) => self.report(&text, format),
            None => self.text(&text),
        };
        std::fs::write(target, scrubbed).with_context(|| format!("writing {}", target.display()))
    }
}

/// A test's name in either format: TRX `testName` and `<UnitTest name>`, JUnit `<testcase name>`.
static TEST_NAMES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:\btestName=|<UnitTest\b[^>]*?\sname=|<testcase\b[^>]*?\sname=)"[^"]*""#).expect("valid regex")
});

/// The host attribute of either format: TRX `computerName`, JUnit `hostname`.
static HOST: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\b(computerName|hostname)="[^"]*""#).expect("valid regex"));

/// TRX names the run `user@host date`.
static RUN_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(<TestRun\b[^>]*?\sname=")[^"]*(")"#).expect("valid regex"));

/// TRX's deployment folder is `user_host_date`.
static DEPLOYMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\b(runDeploymentRoot)="[^"]*""#).expect("valid regex"));

/// A data collector's attachments (coverlet's coverage, a blame dump): links into the results directory, which never
/// enters evidence, under the host's name. The coverage is read into the footprint instead.
static COLLECTORS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)[ \t]*<CollectorDataEntries>.*?</CollectorDataEntries>\r?\n?").expect("valid regex")
});

/// The GUIDs a TRX mints per run: the run, its settings, each execution and its results folder. Test ids and test
/// list ids are stable across runs and stay as they are.
static RUN_IDS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?:<TestRun\b[^>]*?\sid=|<TestSettings\b[^>]*?\sid=|\bexecutionId=|\brelativeResultsDirectory=)"([0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12})""#,
    )
    .expect("valid regex")
});

/// Replaces every run GUID, wherever it appears, with a sequence number in order of first appearance, keeping the
/// cross-references between results and entries intact.
fn renumber_run_ids(text: &str) -> String {
    let mut ids: Vec<String> = Vec::new();
    for capture in RUN_IDS.captures_iter(text) {
        let id = capture[1].to_ascii_lowercase();
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids.iter().enumerate().fold(text.to_string(), |text, (index, id)| {
        let stable = format!("00000000-0000-0000-0000-{:012}", index + 1);
        let pattern = Regex::new(&format!("(?i){}", regex::escape(id))).expect("escaped literal");
        pattern.replace_all(&text, stable.as_str()).into_owned()
    })
}

/// Matches `path` case-insensitively (TRX lowercases some paths) with either separator (a Windows runner may
/// write either).
fn path_pattern(path: &str) -> Regex {
    let body: String = path
        .chars()
        .map(|character| match character {
            '/' | '\\' => r"[/\\]".to_string(),
            other => regex::escape(&other.to_string()),
        })
        .collect();
    Regex::new(&format!("(?i){body}")).expect("escaped path")
}

/// Replaces matches that are whole paths: not the tail of a longer name (`/var/tmp` is not `/tmp`) and not the head
/// of one (`/home/ann` is not `/home/anna`).
fn replace_bounded(text: &str, pattern: &Regex, with: &str) -> String {
    let name = |character: char| character.is_alphanumeric() || matches!(character, '_' | '.' | '-');
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for found in pattern.find_iter(text) {
        let before = text[..found.start()].chars().next_back();
        let after = text[found.end()..].chars().next();
        if before.is_some_and(|c| name(c) || c == '/' || c == '\\') || after.is_some_and(name) {
            continue;
        }
        out.push_str(&text[last..found.start()]);
        out.push_str(with);
        last = found.end();
    }
    out.push_str(&text[last..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scrub(checkout: &str) -> Scrub {
        Scrub {
            paths: [(checkout, "{root}"), ("/tmp", "{tmp}"), ("/home/ann", "~")]
                .into_iter()
                .map(|(path, placeholder)| (path_pattern(path), placeholder))
                .collect(),
        }
    }

    const TRX: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<TestRun id="6c0a433a-ac2b-41a3-bc4f-02a606c2dee9" name="ann@box-7 2026-09-24 02:18:24" xmlns="http://microsoft.com/schemas/VisualStudio/TeamTest/2010">
  <TestSettings name="default" id="5fc244d6-e89b-49a9-a8f4-eabc1de49f8a">
    <Deployment runDeploymentRoot="ann_box-7_2026-09-24_02_18_24" />
  </TestSettings>
  <Results>
    <UnitTestResult executionId="105FE853-5d8b-42a4-b600-b482b99eee5f" testId="d1ec8f2e-ebfb-b58a-f9d7-7605033429b1" testName="FM-1: a deposit /tmp is read back" computerName="box-7" duration="00:00:00.0121809" outcome="Failed" testListId="8c84fa94-04c1-424b-9868-57a2d4851a1d" relativeResultsDirectory="105fe853-5d8b-42a4-b600-b482b99eee5f">
      <Output><ErrorInfo><Message>expected 10 but was 0</Message><StackTrace>at Specs.S0001.DepositSpec.Reads() in /tmp/skies-red-oDF7Uh/checkout/app/.specs/0001-deposit/e2e/DepositSpec.cs:line 23</StackTrace></ErrorInfo></Output>
    </UnitTestResult>
  </Results>
  <TestDefinitions>
    <UnitTest name="FM-1: a deposit /tmp is read back" storage="/tmp/skies-red-odf7uh/checkout/app/tests/bin/debug/tests.dll" id="d1ec8f2e-ebfb-b58a-f9d7-7605033429b1">
      <Execution id="105fe853-5d8b-42a4-b600-b482b99eee5f" />
    </UnitTest>
  </TestDefinitions>
  <TestEntries>
    <TestEntry testId="d1ec8f2e-ebfb-b58a-f9d7-7605033429b1" executionId="105fe853-5d8b-42a4-b600-b482b99eee5f" testListId="8c84fa94-04c1-424b-9868-57a2d4851a1d" />
  </TestEntries>
  <ResultSummary outcome="Failed"><Output><StdOut>Content root path: /tmp/skies-red-oDF7Uh/checkout/app/Api
cache at /home/ann/.nuget/packages and /var/tmp/x and /home/anna/y</StdOut></Output>
    <CollectorDataEntries>
      <Collector agentName="box-7" uri="datacollector://microsoft/CoverletCodeCoverage/1.0" collectorDisplayName="XPlat code coverage">
        <UriAttachments><UriAttachment><A href="box-7/coverage.cobertura.xml"></A></UriAttachment></UriAttachments>
      </Collector>
    </CollectorDataEntries>
  </ResultSummary>
</TestRun>
"#;

    #[test]
    fn a_trx_report_keeps_its_results_and_loses_the_machine() {
        let scrubbed = scrub("/tmp/skies-red-oDF7Uh/checkout").report(TRX, Format::Trx);

        assert!(!scrubbed.contains("box-7") && !scrubbed.contains("ann@") && !scrubbed.contains("oDF7Uh"));
        assert!(
            !scrubbed.to_lowercase().contains("odf7uh"),
            "lowercased paths are matched too"
        );
        assert!(scrubbed.contains(r#"<TestRun id="00000000-0000-0000-0000-000000000001" name="{run}""#));
        assert!(scrubbed.contains(r#"<TestSettings name="default" id="00000000-0000-0000-0000-000000000002">"#));
        assert!(scrubbed.contains(r#"runDeploymentRoot="{run}""#));
        assert_eq!(
            scrubbed.matches("00000000-0000-0000-0000-000000000003").count(),
            4,
            "{scrubbed}"
        );
        assert!(scrubbed.contains(r#"computerName="{machine}""#));
        assert!(scrubbed.contains("in {root}/app/.specs/0001-deposit/e2e/DepositSpec.cs:line 23"));
        assert!(scrubbed.contains(r#"storage="{root}/app/tests/bin/debug/tests.dll""#));
        assert!(scrubbed.contains("Content root path: {root}/app/Api"));
        assert!(scrubbed.contains("cache at ~/.nuget/packages and /var/tmp/x and /home/anna/y"));
        assert!(
            !scrubbed.contains("CollectorDataEntries") && !scrubbed.contains("coverage.cobertura"),
            "collector attachments point outside evidence"
        );

        // What the run proved is untouched.
        for kept in [
            r#"testName="FM-1: a deposit /tmp is read back""#,
            r#"duration="00:00:00.0121809""#,
            r#"outcome="Failed""#,
            "<Message>expected 10 but was 0</Message>",
            r#"testId="d1ec8f2e-ebfb-b58a-f9d7-7605033429b1""#,
            r#"testListId="8c84fa94-04c1-424b-9868-57a2d4851a1d""#,
        ] {
            assert!(scrubbed.contains(kept), "{kept} survives");
        }
        assert!(roxmltree::Document::parse(&scrubbed).is_ok(), "still well-formed XML");
    }

    #[test]
    fn a_junit_report_keeps_its_results_and_loses_the_machine() {
        let junit = r#"<testsuites tests="1" time="0.25">
  <testsuite name="app/.specs/0006/e2e/Deposit.test.tsx" timestamp="2026-09-24T05:47:20.750Z" hostname="box-7" tests="1">
    <testcase classname="app/.specs/0006/e2e/Deposit.test.tsx" name="FM-1: an invalid submit is blocked" time="0.101665677">
      <failure message="expected true">AssertionError at C:\Work\Repo\app\src\Deposit.tsx:12</failure>
    </testcase>
  </testsuite>
</testsuites>"#;

        let scrubbed = scrub("c:/work/repo").report(junit, Format::JUnit);

        assert!(scrubbed.contains(r#"hostname="{machine}""#));
        assert!(
            scrubbed.contains(r"AssertionError at {root}\app\src\Deposit.tsx:12"),
            "{scrubbed}"
        );
        for kept in [
            r#"name="FM-1: an invalid submit is blocked" time="0.101665677""#,
            r#"<failure message="expected true">"#,
            r#"timestamp="2026-09-24T05:47:20.750Z""#,
        ] {
            assert!(scrubbed.contains(kept), "{kept} survives");
        }
        assert!(roxmltree::Document::parse(&scrubbed).is_ok());
    }

    #[test]
    fn a_log_loses_only_its_paths() {
        let log = "error CS0246 in /tmp/skies-red-a1/checkout/Api/Wallet.cs(4,1) on box-7\ncp /tmp/a /tmp/b\n";
        assert_eq!(
            scrub("/tmp/skies-red-a1/checkout").text(log),
            "error CS0246 in {root}/Api/Wallet.cs(4,1) on box-7\ncp {tmp}/a {tmp}/b\n"
        );
    }

    #[test]
    fn a_root_too_short_to_be_specific_is_ignored() {
        let scrub = Scrub::new(&[Path::new("/")]);
        assert!(scrub.paths.iter().all(|(pattern, _)| !pattern.is_match("/")));
    }
}
