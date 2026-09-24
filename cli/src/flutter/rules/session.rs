//! The session rules (SKYFL016, 017, 029), twins of SKYFE016, 017, and 029: a token is written only through the
//! session seam, a guard's redirect never decides on an auth boolean, and the refresh rotation has one door.
//!
//! Each matches the session meaning, not a word: a storage key is token-ish when one of its words (camelCase,
//! snake_case, kebab, or dotted) is `token`, `jwt`, `session`, or `auth`, so `authorName` and `authority` are not;
//! and a `refresh` call is a rotation only on a session/auth receiver or the generated client, so Riverpod's
//! `ref.refresh(provider)` or a list's `controller.refresh()` is not.

use super::checks::Report;
use super::facts::{Call, Facts};

/// The words of a key or a receiver that denote a session credential.
const SESSION_WORDS: [&str; 5] = ["token", "tokens", "jwt", "session", "auth"];

/// The token setters the seam owns (`setAccessToken`, `setToken`, `setSession`), the SKYFE016 set.
const TOKEN_SETTERS: [&str; 3] = ["setAccessToken", "setToken", "setSession"];

/// The rotation calls by name, whatever the receiver.
const ROTATIONS: [&str; 3] = ["refreshToken", "refreshAccessToken", "refreshSession"];

/// The auth booleans a guard must not redirect on: the SKYFE017 `AUTH_BOOL` set.
const AUTH_BOOLS: [&str; 8] = [
    "isAuthenticated",
    "authenticated",
    "isAuthed",
    "authed",
    "isLoggedIn",
    "loggedIn",
    "isSignedIn",
    "signedIn",
];

/// The lowercase words of an identifier or key: `accessToken` → `access token`, `auth_token` → `auth token`,
/// `_sessionSeam` → `session seam`.
pub(super) fn words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    let mut previous_lower = false;
    for ch in text.chars() {
        if !ch.is_ascii_alphanumeric() {
            out.extend((!word.is_empty()).then(|| std::mem::take(&mut word)));
            previous_lower = false;
            continue;
        }
        if ch.is_ascii_uppercase() && previous_lower {
            out.push(std::mem::take(&mut word));
        }
        previous_lower = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        word.push(ch.to_ascii_lowercase());
    }
    out.extend((!word.is_empty()).then_some(word));
    out
}

/// Whether a key or a receiver names a session credential by one of its words.
pub(super) fn session_ish(text: &str) -> bool {
    words(text).iter().any(|word| SESSION_WORDS.contains(&word.as_str()))
}

pub(super) fn session_rules(report: &mut Report, facts: &Facts, session_door: bool, routing: bool) {
    guard_tristate(report, facts, routing);
    if session_door {
        return;
    }
    let token_write = facts.calls.iter().find(|call| {
        TOKEN_SETTERS.contains(&call.name.as_str())
            || (["write", "setString"].contains(&call.name.as_str()) && key(call).is_some_and(session_ish))
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
        .or_else(|| facts.calls.iter().find(|call| is_rotation(call)).map(|c| c.line));
    if let Some(line) = rotation {
        report.add(
            "refresh-one-door",
            Some(line),
            "refresh rotation is consumed outside the session/client seam",
        );
    }
}

/// The storage key of a `write(key: …)` / `setString('…', …)` call, when it is a literal.
fn key(call: &Call) -> Option<&str> {
    call.args
        .iter()
        .find(|arg| arg.label.is_none() || arg.label.as_deref() == Some("key"))
        .and_then(|arg| arg.string.as_deref())
}

/// A session rotation: a rotation call by name, a `refresh` on the generated client or on a session/auth
/// receiver, or a hand-rolled POST to a refresh route (SKYFE029's `.post("/auth/refresh")`).
fn is_rotation(call: &Call) -> bool {
    if ROTATIONS.contains(&call.name.as_str()) {
        return true;
    }
    if call.name == "refresh" {
        return call.is_operation || call.receiver.as_deref().is_some_and(session_ish);
    }
    call.name == "post"
        && call
            .positional()
            .next()
            .and_then(|arg| arg.string.as_deref())
            .is_some_and(|url| words(url).iter().any(|word| word == "refresh"))
}

/// SKYFL017: a redirect decided on an auth boolean. GoRouter's `redirect:` callback reading one, or, in a route or
/// guard file, an `if` whose condition reads one (the SKYFE017 `if (!isAuthenticated) return <Navigate/>`).
fn guard_tristate(report: &mut Report, facts: &Facts, routing: bool) {
    let in_redirect = facts
        .bindings
        .iter()
        .find(|b| b.name == "redirect" && b.identifiers.iter().any(|id| AUTH_BOOLS.contains(&id.as_str())))
        .map(|b| b.line);
    let in_guard = facts
        .condition_identifiers
        .iter()
        .find(|id| routing && AUTH_BOOLS.contains(&id.value.as_str()))
        .map(|id| id.line);
    if let Some(line) = in_redirect.or(in_guard) {
        report.add(
            "guard-tristate",
            Some(line),
            "route guard collapses session loading into a boolean",
        );
    }
}
