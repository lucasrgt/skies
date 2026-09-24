//! The rule predicates. Each reads the syntax facts of one file (plus, for a few, its paired file or the whole
//! package) and keeps the 4.x message, so findings read the same as before the port.

use std::collections::HashMap;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use super::facts::{Call, Facts};
use super::{Source, code};
use crate::doctor::{Finding, Severity};

const PLATFORM_PACKAGES: [&str; 12] = [
    "camera",
    "connectivity_plus",
    "device_info_plus",
    "file_picker",
    "flutter_secure_storage",
    "geolocator",
    "image_picker",
    "package_info_plus",
    "path_provider",
    "permission_handler",
    "shared_preferences",
    "url_launcher",
];
const MUTATIONS: [&str; 8] = [
    "submit", "save", "create", "update", "delete", "remove", "deposit", "withdraw",
];
const NAVIGATION: [&str; 4] = ["go", "push", "pushReplacement", "replace"];

/// Finds the line of the first import whose URI satisfies a predicate.
type ImportLine<'a> = dyn Fn(&dyn Fn(&str) -> bool) -> Option<usize> + 'a;

struct Report<'a> {
    source: &'a Source,
    findings: Vec<Finding>,
}

impl Report<'_> {
    fn add(&mut self, rule: &str, line: Option<usize>, message: &str) {
        self.push(rule, Severity::Error, line, message);
    }

    fn warn(&mut self, rule: &str, line: Option<usize>, message: &str) {
        self.push(rule, Severity::Warning, line, message);
    }

    fn push(&mut self, rule: &str, severity: Severity, line: Option<usize>, message: &str) {
        self.findings.push(Finding::new(
            code(rule),
            severity,
            self.source.path.clone(),
            line,
            message.to_string(),
        ));
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

    if !role.test {
        let mock = ["__mocks__", "fixtures", "mockito", "mocktail", "msw"];
        if let Some(line) = import(&|uri| mock.iter().any(|m| uri.contains(m))) {
            report.add("no-mock", Some(line), "production code imports a mock or fixture");
        }
    }
    if role.view {
        let transport = import(&|uri| uri.starts_with("package:dio/")).or_else(|| {
            facts
                .first_identifier(&["Dio", "SkiesClient", "executeSkiesRequest"])
                .map(|i| i.line)
        });
        if let Some(line) = transport {
            report.add("view-purity", Some(line), "View reaches transport or client behavior");
        }
        let model = sibling(&source.path, "_view.dart", "_view_model.dart");
        let paired = sources.get(model.as_path());
        if paired.is_none() && role.lib {
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
    if !role.model && !role.session_door {
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
    session_rules(&mut report, facts, role.session_door);
    if role.routing
        && let Some(id) = facts.first_identifier(&["isAuthenticated"])
    {
        report.add(
            "guard-tristate",
            Some(id.line),
            "route guard collapses session loading into a boolean",
        );
    }
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
    if let Some(binding) = facts.bindings.iter().find(|b| {
        b.name == "onSuccess"
            && b.identifiers
                .iter()
                .any(|id| ["refetch", "reload", "invalidate"].iter().any(|w| id.contains(w)))
    }) {
        report.warn(
            "no-manual-refetch",
            Some(binding.line),
            "success callback only repeats global invalidation",
        );
    }
    if role.ui
        && let Some(call) = facts
            .calls_named(&["TextFormField"])
            .find(|call| call.named("validator").is_none() && call.named("errorText").is_none())
    {
        report.warn(
            "field-error-surface",
            Some(call.line),
            "form field exposes no validation error surface",
        );
    }
    report.findings
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
    let platform = import(&|uri| {
        uri == "dart:io"
            || PLATFORM_PACKAGES
                .iter()
                .any(|name| uri.starts_with(&format!("package:{name}/")))
    });
    if let Some(line) = platform {
        report.add(
            "viewmodel-platform-agnostic",
            Some(line),
            "ViewModel imports a device capability instead of an injected port",
        );
    }
    if !facts.generics.contains("AsyncState") {
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
        report.warn(
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

fn navigation_rules(report: &mut Report, facts: &Facts) {
    let imperative = facts.calls.iter().find(|call| {
        ["pushReplacement", "go", "replace"].contains(&call.name.as_str())
            && call
                .enclosing
                .iter()
                .any(|outer| outer == "addPostFrameCallback" || outer == "addListener")
    });
    if let Some(call) = imperative {
        report.add(
            "declarative-redirect",
            Some(call.line),
            "state-driven redirect runs imperatively after render",
        );
    }
    let unguarded = facts.index_reads.iter().find(|read| {
        let key = read.index.trim_end_matches(['\'', '"']);
        (read.object.ends_with("pathParameters") || read.object.ends_with("queryParameters"))
            && (key.ends_with("id") || key.ends_with("Id"))
    });
    if let Some(read) = unguarded
        && facts.calls_named(&["requiredParam"]).next().is_none()
    {
        report.add(
            "route-param-guard",
            Some(read.line),
            "required route id is read without requiredParam",
        );
    }
    let bare_back = facts.calls_named(&["pop"]).find(|call| {
        call.receiver
            .as_deref()
            .is_some_and(|r| r == "Navigator" || r == "context" || r.starts_with("Navigator.of("))
    });
    if let Some(call) = bare_back {
        report.add(
            "safe-back",
            Some(call.line),
            "bare back navigation has no deep-link fallback",
        );
    }
    let open = facts.calls_named(&NAVIGATION).find(|call| {
        call.positional()
            .next()
            .and_then(|arg| arg.identifier.as_deref())
            .is_some_and(|id| ["returnTo", "next", "redirect"].contains(&id))
    });
    if let Some(call) = open {
        report.add(
            "no-open-redirect",
            Some(call.line),
            "navigation consumes a URL-derived target without an allowlist",
        );
    }
    let escaped = facts.calls.iter().find(|call| {
        let receiver = call.receiver.as_deref().unwrap_or_default();
        let navigates = (receiver == "context" && ["go", "push", "replace"].contains(&call.name.as_str()))
            || receiver == "Navigator"
            || receiver.starts_with("Navigator.");
        navigates
            && call.args.iter().any(|arg| {
                arg.cast
                    .as_deref()
                    .is_some_and(|t| ["dynamic", "Object", "string", "String"].contains(&t))
            })
    });
    if let Some(call) = escaped {
        report.add(
            "typed-navigation",
            Some(call.line),
            "navigation target escapes its typed route through a cast",
        );
    }
}

fn session_rules(report: &mut Report, facts: &Facts, session_door: bool) {
    if session_door {
        return;
    }
    let token_write = facts.calls.iter().find(|call| {
        call.name == "setAccessToken"
            || (["write", "setString"].contains(&call.name.as_str())
                && call
                    .args
                    .iter()
                    .find(|arg| arg.label.is_none() || arg.label.as_deref() == Some("key"))
                    .and_then(|arg| arg.string.as_deref())
                    .is_some_and(|key| key.to_ascii_lowercase().contains("token")))
    });
    if let Some(call) = token_write {
        report.add(
            "session-one-door",
            Some(call.line),
            "session token is written outside the session seam",
        );
    }
    let rotation = facts
        .first_identifier(&["refreshSession", "bootstrapSession"])
        .map(|id| id.line)
        .or_else(|| facts.calls_named(&["refresh"]).next().map(|c| c.line));
    if let Some(line) = rotation {
        report.add(
            "refresh-one-door",
            Some(line),
            "refresh rotation is consumed outside the session/client seam",
        );
    }
}

/// Unfinished-work markers are a text property of comments, so this one is a regex over comment nodes only;
/// `UnimplementedError(...)` is a real call.
fn placeholder(report: &mut Report, facts: &Facts) {
    static MARKER: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)\b(?:TODO|FIXME|HACK|XXX|wire later)\b").expect("static pattern"));
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
        return vec![Finding::new(
            code("mutation-defaults"),
            Severity::Error,
            "<project>".into(),
            None,
            message,
        )];
    }
    Vec::new()
}

/// `lib/x/wallets_view.dart` → `lib/x/wallets_view_model.dart`.
fn sibling(path: &Path, suffix: &str, replacement: &str) -> std::path::PathBuf {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    path.with_file_name(format!("{}{replacement}", name.trim_end_matches(suffix)))
}
