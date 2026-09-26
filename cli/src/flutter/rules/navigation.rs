//! The navigation rules (SKYFL015, 018, 019, 022, 030): declarative redirects, guarded route ids, a Back that
//! survives a deep link, allowlisted redirect targets, and typed routes.

use super::checks::Report;
use super::facts::{Call, Facts};

const NAVIGATION: [&str; 4] = ["go", "push", "pushReplacement", "replace"];

pub(super) fn navigation_rules(report: &mut Report, facts: &Facts) {
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
    let bare_back = facts
        .calls_named(&["pop"])
        .find(|call| is_route_pop(call) && !closes_what_it_opened(call, facts) && !guarded_back(facts));
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

/// A pop of the navigation stack: `Navigator.pop(...)`, `Navigator.of(context).pop()`, go_router's `context.pop()`.
fn is_route_pop(call: &Call) -> bool {
    call.receiver
        .as_deref()
        .is_some_and(|r| r == "Navigator" || r == "context" || r.starts_with("Navigator.of("))
}

/// A pop that closes something this code opened, not a Back affordance. A deep link cannot land on an overlay or on a
/// page pushed imperatively, so such a pop always has somewhere to go. Recognized when the pop:
/// - returns a result to the route that pushed it (`pop(true)`, `pop(code)`);
/// - runs inside the builder of a dialog, sheet, or modal (`showDialog(...)`, `showAppModal(...)`);
/// - sits in a file that pushes its own page with `Navigator.push(...)` (a scanner, a picker);
/// - pops an overlay's own route context (`Navigator.of(dialogContext)`, not the page's `context`) in a file that opens
///   overlays (`show*`).
fn closes_what_it_opened(call: &Call, facts: &Facts) -> bool {
    let receiver = call.receiver.as_deref().unwrap_or_default();
    let result = call.positional().nth(usize::from(receiver == "Navigator"));
    let pushes_page = facts
        .calls
        .iter()
        .any(|c| c.name.starts_with("push") && c.receiver.as_deref().is_some_and(|r| r.starts_with("Navigator")));
    let opens_overlay = facts.calls.iter().any(|c| c.name.starts_with("show"));
    let overlay_context = receiver.starts_with("Navigator.of(") && receiver != "Navigator.of(context)";
    result.is_some()
        || call.enclosing.iter().any(|outer| outer.starts_with("show"))
        || pushes_page
        || (opens_overlay && overlay_context)
}

/// The file already guards its Back with `canPop`: `if (context.canPop()) ... else context.go(fallback)` is the safe
/// shape (the `popOrGo` helper an app keeps in its routing seam), as is a `canPop() ? pop() : go(...)`.
fn guarded_back(facts: &Facts) -> bool {
    facts.calls_named(&["canPop"]).next().is_some()
}
