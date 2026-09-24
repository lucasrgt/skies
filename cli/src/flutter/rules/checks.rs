//! The rule predicates. Each reads the syntax facts of one file (plus, for a few, its paired file or the whole
//! package) and keeps the 4.x message, so findings read the same as before the port.

use std::collections::HashMap;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use super::facts::{Call, Facts};
use super::navigation::navigation_rules;
use super::session::session_rules;
use super::{Source, finding, generated};
use crate::doctor::Finding;

const MUTATIONS: [&str; 8] = [
    "submit", "save", "create", "update", "delete", "remove", "deposit", "withdraw",
];

/// Finds the line of the first import whose URI satisfies a predicate.
type ImportLine<'a> = dyn Fn(&dyn Fn(&str) -> bool) -> Option<usize> + 'a;

pub(super) struct Report<'a> {
    pub(super) source: &'a Source,
    pub(super) findings: Vec<Finding>,
}

impl Report<'_> {
    /// Reports the named rule at its catalogued tier (see [`super::RULES`]).
    pub(super) fn add(&mut self, rule: &str, line: Option<usize>, message: &str) {
        let finding = finding(rule, self.source.path.clone(), line, message.to_string());
        self.findings.push(finding);
    }
}

/// The per-file rules. `sources` gives access to the paired ViewModel of a View.
pub fn file(source: &Source, sources: &HashMap<&Path, &Source>) -> Vec<Finding> {
    let mut report = Report {
        source,
        findings: Vec::new(),
    };
    let (facts, role) = (&source.facts, source.role);
    let import = |pred: &dyn Fn(&str) -> bool| facts.imports.iter().find(|i| pred(&i.value)).map(|i| i.line);
    if generated(&source.path.to_string_lossy().replace('\\', "/")) {
        return Vec::new();
    }
    // Test code (a harness seeding the backend, a test-only route) is not the shipped app: only SKYFL036 reads it.
    if role.test {
        tests_live_in_specs(&mut report, facts, &import);
        return report.findings;
    }

    if !role.test {
        let mock = ["__mocks__", "fixtures", "mockito", "mocktail", "msw"];
        if let Some(line) = import(&|uri| mock.iter().any(|m| uri.contains(m))) {
            report.add("no-mock", Some(line), "production code imports a mock or fixture");
        }
    }
    if role.view || role.view_part {
        let transport = import(&|uri| uri.starts_with("package:dio/")).or_else(|| {
            facts
                .first_identifier(&["Dio", "SkiesClient", "executeSkiesRequest"])
                .map(|i| i.line)
        });
        if let Some(line) = transport {
            report.add("view-purity", Some(line), "View reaches transport or client behavior");
        }
        let model = sibling(&source.path, "_view.dart", "_view_model.dart");
        let paired = sources.get(model.as_path()).filter(|_| role.view);
        if paired.is_none() && role.view && role.lib {
            report.add("view-purity", None, "ViewModel is not co-located");
        }
        if paired.is_some_and(|m| m.facts.generics.contains("AsyncState"))
            && !facts.generics.contains("ResourceBuilder")
        {
            report.add(
                "state-completeness",
                None,
                "View does not route its closed resource through ResourceBuilder",
            );
        }
        hardcoded_copy(&mut report, facts);
    }
    if !role.model && !role.model_part && !role.session_door {
        let operation = facts
            .calls
            .iter()
            .find(|c| c.is_operation || (c.name == "executeSkiesRequest" && c.generic));
        if let Some(call) = operation {
            report.add(
                "data-door",
                Some(call.line),
                "generated operation is consumed outside a ViewModel or infrastructure door",
            );
        }
    }
    if role.model {
        model_rules(&mut report, facts, &import);
    }
    navigation_rules(&mut report, facts);
    session_rules(&mut report, facts, role.session_door, role.routing);
    let hardcoded_url = facts.calls_named(&["BaseOptions", "Dio"]).find(|call| {
        call.named("baseUrl")
            .and_then(|arg| arg.string.as_deref())
            .is_some_and(|url| url.starts_with("http://") || url.starts_with("https://"))
    });
    if let Some(call) = hardcoded_url {
        report.add("configured-base-url", Some(call.line), "API base URL is hardcoded");
    }
    if !role.html_door {
        let html = import(&|uri| uri.starts_with("package:flutter_html/")).or_else(|| {
            facts
                .calls_named(&["Html", "HtmlElementView", "WebViewWidget"])
                .next()
                .map(|c| c.line)
        });
        if let Some(line) = html {
            report.add(
                "raw-html-one-door",
                Some(line),
                "raw HTML rendering occurs outside lib/html",
            );
        }
    }
    if !role.test {
        placeholder(&mut report, facts);
    }
    if role.model || role.model_part {
        manual_refetch(&mut report, facts);
    }
    if role.ui {
        field_error_surface(&mut report, facts);
    }
    tests_live_in_specs(&mut report, facts, &import);
    report.findings
}

/// The runners whose test functions make a Dart file a test file.
const TEST_PACKAGES: [&str; 3] = ["package:test/", "package:flutter_test/", "package:integration_test/"];

/// SKYFL036: every test lives in a spec. A package's own `test/` and `integration_test/` hold no cases; a spec's Dart
/// cases live in the repository's `.specs/<id>-<slug>/e2e/` and the Flutter runner copies them into a hidden folder
/// of the package, which the doctor does not walk. Reported once per file, at the first test call.
fn tests_live_in_specs(report: &mut Report, facts: &Facts, import: &ImportLine) {
    let path = report.source.path.to_string_lossy().replace('\\', "/");
    if path.contains("/.specs/") || import(&|uri| TEST_PACKAGES.iter().any(|p| uri.starts_with(p))).is_none() {
        return;
    }
    let first = facts
        .calls_named(&["test", "testWidgets", "group"])
        .filter(|call| call.receiver.is_none())
        .map(|call| call.line)
        .min();
    if let Some(line) = first {
        report.add(
            "tests-live-in-specs",
            Some(line),
            "tests live in a spec: move this file's cases into .specs/<id>-<slug>/e2e/ and title each after the \
             failure mode it covers (FM-n)",
        );
    }
}

fn hardcoded_copy(report: &mut Report, facts: &Facts) {
    let text_literal = facts
        .calls_named(&["Text", "RichText"])
        .find(|call| call.positional().next().is_some_and(|arg| arg.string.is_some()));
    let labelled = facts.calls.iter().find(|call| {
        call.args.iter().any(|arg| {
            arg.string.is_some()
                && arg
                    .label
                    .as_deref()
                    .is_some_and(|l| ["label", "hintText", "semanticLabel", "tooltip"].contains(&l))
        })
    });
    if let Some(call) = text_literal.or(labelled) {
        report.add(
            "no-hardcoded-copy",
            Some(call.line),
            "View contains user-facing string copy instead of localizations",
        );
    }
}

fn model_rules(report: &mut Report, facts: &Facts, import: &ImportLine) {
    let rendering = import(&|uri| {
        [
            "package:flutter/widgets.dart",
            "package:flutter/material.dart",
            "package:flutter/cupertino.dart",
        ]
        .contains(&uri)
    })
    .or_else(|| {
        facts
            .first_identifier(&["Widget", "BuildContext", "Navigator"])
            .map(|i| i.line)
    });
    if let Some(line) = rendering {
        report.add(
            "viewmodel-render-agnostic",
            Some(line),
            "ViewModel imports or names rendering APIs",
        );
    }
    // SKYFL007, the SKYFE007 twin: a server-backed ViewModel (it calls the generated client or loads anything
    // asynchronously) exposes its states as the closed AsyncState. A purely local one (a wizard's step state) has
    // no load to fail.
    let server_backed = facts
        .calls
        .iter()
        .any(|c| c.is_operation || c.name == "executeSkiesRequest")
        || facts
            .signatures
            .iter()
            .any(|s| s.returns == "Future" || s.returns == "Stream");
    if server_backed && !facts.generics.contains("AsyncState") {
        report.add("mandatory-state", None, "server-backed ViewModel exposes no AsyncState");
    }
    let local_surface = facts.catch_blocks.iter().any(|block| {
        block
            .iter()
            .any(|id| id.contains("AsyncFailure") || id.contains("error") || id.contains("feedback"))
    });
    if let Some(mutation) = mutation(facts)
        && !facts.has_identifier("MutationBoundary")
        && !local_surface
    {
        report.add(
            "mutation-error-surface",
            Some(mutation),
            "mutation has no global or local failure surface",
        );
    }
    let expected = facts
        .calls
        .iter()
        .find(|c| c.named("expectedFailure").is_some_and(|arg| arg.is_true));
    if let Some(call) = expected
        && !local_surface
    {
        report.add(
            "mutation-error-surface",
            Some(call.line),
            "expected failure suppresses global feedback without a modeled local failure surface",
        );
    }
    let validates: Vec<&Call> = facts.calls_named(&["validate"]).filter(|c| c.args.is_empty()).collect();
    let has_invalid_path = facts.has_else
        || facts.calls_named(&["submitOrReveal"]).next().is_some()
        || validates.iter().any(|c| c.in_condition);
    if let Some(call) = validates.first()
        && !has_invalid_path
    {
        report.add(
            "submit-invalid-path",
            Some(call.line),
            "form validation has no explicit invalid path",
        );
    }
}

/// The line of the first mutation command (`Future<...> submit(...)` and friends), if any.
pub fn mutation(facts: &Facts) -> Option<usize> {
    facts
        .signatures
        .iter()
        .find(|s| s.returns == "Future" && MUTATIONS.contains(&s.name.as_str()))
        .map(|s| s.line)
}

/// The refetch calls a success handler repeats although the `MutationBoundary` already invalidates (SKYFE028's
/// `refetch`/`invalidateQueries` set, in Dart's spellings).
const REFETCHES: [&str; 7] = [
    "refetch",
    "reload",
    "refresh",
    "invalidate",
    "invalidateQueries",
    "refetchQueries",
    "resetQueries",
];

/// SKYFL028, the SKYFE028 twin: in a ViewModel, an `onSuccess` callback whose whole body is refetch calls. A handler
/// that does more (navigates, resets a form, hands off an id) is real behavior and never flagged.
fn manual_refetch(report: &mut Report, facts: &Facts) {
    let ritual = facts.bindings.iter().find(|binding| {
        let mut calls = binding.calls(facts).peekable();
        binding.name == "onSuccess"
            && calls.peek().is_some()
            && calls.all(|call| REFETCHES.contains(&call.name.as_str()))
    });
    if let Some(binding) = ritual {
        report.add(
            "no-manual-refetch",
            Some(binding.line),
            "success callback only repeats global invalidation",
        );
    }
}

/// SKYFL032, the SKYFE032 twin: a validated field shows its error where the control is. A `TextFormField` renders
/// its own validator's error, so a field without a validator has none to lose; the error is lost in the app's field
/// primitive (`lib/ui/`, the `AppInput` every screen uses) when it builds the `TextFormField` without passing a
/// `validator`, a `forceErrorText`, or a decoration `errorText` through. That primitive is the Flutter spelling of
/// the web's `<Controller render>`, where a resolver's error reaches the field only through `fieldState`.
fn field_error_surface(report: &mut Report, facts: &Facts) {
    let blind = facts.calls_named(&["TextFormField"]).find(|field| {
        field.named("validator").is_none()
            && field.named("forceErrorText").is_none()
            && field.named("errorText").is_none()
            && !facts.within(field).any(|inner| inner.named("errorText").is_some())
    });
    if let Some(call) = blind {
        report.add(
            "field-error-surface",
            Some(call.line),
            "form field exposes no validation error surface",
        );
    }
}

/// Unfinished-work markers are a text property of comments, so this one is a regex over comment nodes only;
/// `UnimplementedError(...)` is a real call. The markers are the conventional uppercase ones, as SKYFE023 reads them: a
/// lowercase "todo" is a word (Portuguese "all"), which calibrating the web twin on a real app found.
fn placeholder(report: &mut Report, facts: &Facts) {
    static MARKER: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\b(?:TODO|FIXME|HACK|XXX|[Ww]ire later)\b").expect("static pattern"));
    let line = facts
        .comments
        .iter()
        .find(|comment| MARKER.is_match(&comment.value))
        .map(|comment| comment.line)
        .or_else(|| facts.calls_named(&["UnimplementedError"]).next().map(|call| call.line));
    if let Some(line) = line {
        report.add(
            "no-placeholder",
            Some(line),
            "production code contains an unfinished placeholder",
        );
    }
}

/// Package-wide: write features need one configured `MutationBoundary` somewhere in production code.
pub fn project(sources: &[Source]) -> Vec<Finding> {
    let writes = sources.iter().any(|s| s.role.model && mutation(&s.facts).is_some());
    let boundary = sources
        .iter()
        .any(|s| !s.role.test && s.facts.calls_named(&["MutationBoundary"]).next().is_some());
    if writes && !boundary {
        let message = "write features exist without one configured MutationBoundary".to_string();
        return vec![finding("mutation-defaults", "<project>".into(), None, message)];
    }
    Vec::new()
}

/// `lib/x/wallets_view.dart` → `lib/x/wallets_view_model.dart`.
fn sibling(path: &Path, suffix: &str, replacement: &str) -> std::path::PathBuf {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    path.with_file_name(format!("{}{replacement}", name.trim_end_matches(suffix)))
}
