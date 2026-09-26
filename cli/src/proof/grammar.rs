//! The one failure-mode grammar, shared by spec.md, test cases, and ctx citations.
//!
//! Defined once here, and kept identical to the spec half of SKY0005 (`SpecCitations` in the Roslyn doctor), so a
//! failure mode the engine sees is the failure mode a ctx citation resolves against.

use std::sync::LazyLock;

use regex::Regex;

use super::report::FmId;

/// The grammar (a ctx.md cites a mode as `` `0002-withdraw#FM-2` ``, which `impact` and SKY0005 read):
///
/// - a failure mode is a `- FM-<n> <text>` (or `* FM-<n>: <text>`) bullet under `## Failure modes` in spec.md;
/// - a case proves it when its title starts with `FM-<n>:` or `FM-<n> `, the title being the name after the last
///   ` > ` or ` › ` (vitest and Playwright prefix the describe blocks), or when its method name starts with `FM<n>_`
///   (a .NET test without a DisplayName, reported as `Specs.S0001.DepositSpec.FM2_double_cancel`). Only the title is
///   read, look-alikes included: a describe block's name never names a mode nor makes its cases look-alikes.
///
/// `FM` is upper case and the hyphen is required. Anything that starts like an id but is not one (`FM 3`, `fm_3`,
/// `FM3:`, `FM-[x]`) is a look-alike and an error: never silently a mode, never silently nothing.
static SPEC_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*[-*]\s+FM-([0-9]+)(?::|\s|$)").expect("valid regex"));
static TITLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^FM-([0-9]+)(?::|\s|$)").expect("valid regex"));
static METHOD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^FM([0-9]+)_").expect("valid regex"));
static LOOK_ALIKE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^fm(?:-|[_ ]?[0-9\[])").expect("valid regex"));
/// A list marker a look-alike may sit behind: `-`, `*`, `1.`, `1)`.
static MARKER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(?:[-*]|[0-9]+[.)])?\s*").expect("valid regex"));

/// Where the grammar is written down for people, for error messages.
pub const GRAMMAR_DOC: &str = "docs/CONVENTIONS.md, \"Specs and proofs\"";

/// A spec.md line: `Ok(Some((id, text)))` for a failure-mode bullet, `Ok(None)` for anything else, and an error for a
/// line that starts like one but is not (`- FM 3`, `- fm_3`, `- FM-[x]`, `FM-3` without its bullet).
pub fn spec_line(line: &str) -> Result<Option<(FmId, &str)>, String> {
    if let Some(capture) = SPEC_LINE.captures(line) {
        let id = capture[1]
            .parse()
            .map(FmId)
            .map_err(|_| format!("`{}`: the number is too large", line.trim()))?;
        let end = capture.get(0).map_or(0, |whole| whole.end());
        return Ok(Some((id, line[end..].trim_start_matches(':').trim())));
    }
    let item = &line[MARKER.find(line).map_or(0, |marker| marker.end())..];
    if LOOK_ALIKE.is_match(item) || TITLE.is_match(item) {
        return Err(format!(
            "`{}` is not a failure-mode line; write `- FM-<n> <what goes wrong>` (a dash, `FM`, a hyphen, the number)",
            line.trim()
        ));
    }
    Ok(None)
}

/// What a test case's name says about the failure modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Named {
    Mode(FmId),
    /// Names no failure mode: a helper, a setup case, a file-level case.
    Nothing,
    /// Starts like a failure-mode id but does not follow the grammar.
    LookAlike,
}

/// What a case's name says, by the grammar above.
pub fn case_mode(name: &str) -> Named {
    let title = [" > ", " › "]
        .iter()
        .filter_map(|separator| name.rfind(separator).map(|at| at + separator.len()))
        .max()
        .map_or(name, |at| &name[at..]);
    // The method comes from the title too, so a describe block that starts like an id (`FM-2: deposit > helper`)
    // neither names a mode nor makes its helper cases look-alikes: only the case's own title is read.
    let signature = title.split('(').next().unwrap_or(title);
    let method = signature.rsplit('.').next().unwrap_or(signature);
    let id = |capture: regex::Captures| capture[1].parse().ok().map(FmId);
    if let Some(id) = TITLE.captures(title).and_then(id) {
        return Named::Mode(id);
    }
    if let Some(id) = METHOD.captures(method).and_then(id) {
        return Named::Mode(id);
    }
    if LOOK_ALIKE.is_match(title) || LOOK_ALIKE.is_match(method) {
        return Named::LookAlike;
    }
    Named::Nothing
}

/// The failure mode a case proves, if its name gives one.
pub fn case_fm(name: &str) -> Option<FmId> {
    match case_mode(name) {
        Named::Mode(id) => Some(id),
        Named::Nothing | Named::LookAlike => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_case_names_its_mode_by_title_or_method() {
        assert_eq!(case_mode("FM-2: double cancel"), Named::Mode(FmId(2)));
        assert_eq!(case_mode("FM-2 double cancel"), Named::Mode(FmId(2)));
        assert_eq!(case_mode("FM-2"), Named::Mode(FmId(2)));
        assert_eq!(case_mode("FM-02: padded"), Named::Mode(FmId(2)));
        assert_eq!(
            case_mode("Deposit screen > FM-3: a failure keeps the form"),
            Named::Mode(FmId(3))
        );
        assert_eq!(case_mode("wallet › FM-4 refused"), Named::Mode(FmId(4)));
        assert_eq!(
            case_mode("Specs.S0001.DepositSpec.FM2_double_cancel"),
            Named::Mode(FmId(2))
        );
        assert_eq!(
            case_mode("Specs.S0001.DepositSpec.FM5_amounts(amount: 1.5)"),
            Named::Mode(FmId(5))
        );
        assert_eq!(
            case_mode("FM-1: amounts like 1.5 and FM-2 mentioned later"),
            Named::Mode(FmId(1)),
            "only the title's start counts"
        );
    }

    #[test]
    fn look_alikes_are_told_apart_from_names_without_a_mode() {
        for name in [
            "FM 3: spaced",
            "fm-3: lower case",
            "FM3: no hyphen",
            "fm_3_underscored",
            "FM-[rejected-update]: named, not numbered",
            "FM-: nothing",
            "Specs.S0001.X.fm2_lower",
            "Screen > FM 2: nested",
        ] {
            assert_eq!(case_mode(name), Named::LookAlike, "{name}");
        }
        for name in [
            "PLATFORM-1 boots",
            "XFM2",
            "setup",
            "Specs.S0001.X.Deposit_FM2",
            "fmt is fine",
            ".specs/0001-a/e2e/a.test.tsx",
        ] {
            assert_eq!(case_mode(name), Named::Nothing, "{name}");
        }
    }

    #[test]
    fn a_describe_block_that_starts_like_an_id_is_not_read() {
        for name in [
            "FM-2: deposit > seeds the wallet",
            "FM 2 spaced describe › helper",
            "fm-2 > Deposit > setup.step",
        ] {
            assert_eq!(case_mode(name), Named::Nothing, "{name}");
        }
        assert_eq!(case_mode("FM-2: deposit > FM-3: refused twice"), Named::Mode(FmId(3)));
        assert_eq!(case_mode("FM-2: deposit > fm_2 helper"), Named::LookAlike);
    }

    #[test]
    fn a_spec_line_is_a_dashed_fm_bullet() {
        assert_eq!(spec_line("- FM-1 a text"), Ok(Some((FmId(1), "a text"))));
        assert_eq!(spec_line("* FM-12: colon"), Ok(Some((FmId(12), "colon"))));
        assert_eq!(spec_line("  - FM-3"), Ok(Some((FmId(3), ""))));
        assert_eq!(spec_line("See FM-7 in passing."), Ok(None));
        assert_eq!(spec_line("- a bullet about fmt"), Ok(None));
        for line in [
            "- FM 3 spaced",
            "- fm_3 underscored",
            "- FM3 glued",
            "- fm-3 lower",
            "- FM-[rejected-update]: named",
            "FM-3 without a bullet",
            "1. FM-3 numbered",
        ] {
            let error = spec_line(line).unwrap_err();
            assert!(error.contains("- FM-<n>"), "{line}: {error}");
        }
    }
}
