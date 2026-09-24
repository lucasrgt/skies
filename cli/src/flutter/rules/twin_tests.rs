//! Regression cases from the rule audit: the twin rules mean what their SKYFE twins mean (016, 017, 028, 029, 032),
//! the refresh door does not catch every `refresh`, taste rules are warnings, and the `skies-ignore` hatch is narrow,
//! reasoned, and visible.

use super::{analyze, diagnose};
use crate::doctor::Severity;

fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("lib")).unwrap();
    for (path, source) in files {
        let path = dir.path().join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, format!("{source}\n")).unwrap();
    }
    dir
}

fn codes(files: &[(&str, &str)]) -> Vec<String> {
    let dir = project(files);
    diagnose(dir.path()).unwrap().into_iter().map(|f| f.code).collect()
}

fn fires(code: &str, path: &str, source: &str) -> bool {
    codes(&[(path, source)]).iter().any(|c| c == code)
}

const HELPER: &str = "lib/features/x/helper.dart";
const MODEL: &str = "lib/features/x/x_view_model.dart";

#[test]
fn skyfl029_is_the_session_rotation_not_any_refresh() {
    for quiet in [
        "void f(WidgetRef ref) => ref.refresh(walletsProvider);",
        "Future<void> f() => _refreshController.refresh();",
        "Future<void> f() => indicator.refresh();",
        "Future<void> f() => dio.post('/feeds/reload');",
    ] {
        assert!(!fires("SKYFL029", HELPER, quiet), "flagged a non-rotation: {quiet}");
    }
    for rotation in [
        "Future<void> f() => refreshSession();",
        "Future<void> f() => session.refresh();",
        "Future<void> f() => _authClient.refresh();",
        "Future<void> f() => client.api.getAuthApi().refresh(body);",
        "Future<void> f() => tokens.refreshAccessToken();",
        "Future<void> f() => dio.post('/auth/refresh');",
    ] {
        assert!(fires("SKYFL029", HELPER, rotation), "missed a rotation: {rotation}");
    }
    assert!(!fires(
        "SKYFL029",
        "lib/session.dart",
        "Future<void> f() => session.refresh();"
    ));
}

#[test]
fn skyfl016_token_keys_are_whole_words_like_skyfe016() {
    for quiet in [
        "void f() => prefs.setString('authorName', name);",
        "void f() => prefs.setString('authority', value);",
        "void f() => storage.write(key: 'theme', value: v);",
    ] {
        assert!(!fires("SKYFL016", HELPER, quiet), "flagged a non-token key: {quiet}");
    }
    for write in [
        "void f() => storage.write(key: 'accessToken', value: t);",
        "void f() => prefs.setString('auth_token', t);",
        "void f() => prefs.setString('session', s);",
        "void f() => prefs.setString('jwt', s);",
        "void f() => setSession(s);",
    ] {
        assert!(fires("SKYFL016", HELPER, write), "missed a token write: {write}");
    }
}

#[test]
fn skyfl017_is_a_redirect_decided_on_an_auth_boolean() {
    let redirect = "final router = GoRouter(redirect: (context, state) => auth.isAuthenticated ? null : '/login');";
    assert!(fires("SKYFL017", "lib/app.dart", redirect));
    let guard = "String? f() { if (!session.isLoggedIn) return '/login'; return null; }";
    assert!(fires("SKYFL017", "lib/routes/account_guard.dart", guard));
    // Reading the boolean to render (a badge, a menu) is not a redirect decision.
    let display = "Widget f() => Text(auth.isAuthenticated ? a : b);";
    assert!(!fires("SKYFL017", "lib/routes/menu_route.dart", display));
    let tristate = "final router = GoRouter(redirect: (context, state) => guardSession(session.state, '/login'));";
    assert!(!fires("SKYFL017", "lib/app.dart", tristate));
}

#[test]
fn skyfl028_flags_only_a_handler_whose_whole_body_refetches() {
    let head = "AsyncState<int> state;\n";
    assert!(fires(
        "SKYFL028",
        MODEL,
        &format!("{head}final onSuccess = () {{ reload(); }};")
    ));
    let more = format!("{head}final onSuccess = () {{ reload(); router.go('/done'); }};");
    assert!(!fires("SKYFL028", MODEL, &more));
    // Outside a ViewModel it is not a mutation handler, as SKYFE028 reads only `*.viewModel.ts`.
    assert!(!fires("SKYFL028", HELPER, "final onSuccess = () { reload(); };"));
}

#[test]
fn skyfl032_the_field_primitive_passes_the_error_through() {
    let blind = "Widget build() => TextFormField(controller: c);";
    assert!(fires("SKYFL032", "lib/ui/app_input.dart", blind));
    // An optional note field with no validator has no validation error to lose (calibration: two in one app).
    assert!(!fires("SKYFL032", "lib/features/x/x_view.dart", blind));
    for surfaced in [
        "Widget build() => TextFormField(validator: validate);",
        "Widget build() => TextFormField(forceErrorText: error);",
        "Widget build() => TextFormField(decoration: InputDecoration(errorText: error));",
    ] {
        assert!(!fires("SKYFL032", "lib/ui/app_input.dart", surfaced), "{surfaced}");
    }
}

#[test]
fn skyfl007_asks_a_server_backed_view_model_for_async_state() {
    let local = "final class StepViewModel extends ChangeNotifier { int step = 0; void next() { step++; } }";
    assert!(!fires("SKYFL007", MODEL, local));
    let loads = "final class XViewModel extends ChangeNotifier { List<int> items = []; Future<void> load() async {} }";
    assert!(fires("SKYFL007", MODEL, loads));
    let reads = "final class XViewModel { void load() => client.api.getWalletApi().listWallets(); }";
    assert!(fires("SKYFL007", MODEL, reads));
}

#[test]
fn taste_rules_are_warnings() {
    let dir = project(&[("lib/helper.dart", "// TODO wire later")]);
    let findings = diagnose(dir.path()).unwrap();
    assert_eq!(findings.len(), 1);
    assert_eq!(
        (findings[0].code.as_str(), findings[0].severity),
        ("SKYFL023", Severity::Warning)
    );
    // A lowercase "todo" is a word (Portuguese "all"), not a marker.
    assert!(!fires("SKYFL023", "lib/helper.dart", "// Reexporta para todo o app."));
}

#[test]
fn a_reasoned_skies_ignore_suppresses_one_rule_visibly() {
    let dir = project(&[
        (
            "lib/features/x/legacy.dart",
            "// skies-ignore: SKYFL029 the legacy client rotates on its own until the port\n\
             Future<void> f() => refreshSession();\n\
             Future<void> g() => refreshSession(); // skies-ignore: SKYFL029 same legacy client\n\
             Future<void> h() => setAccessToken(t); // skies-ignore: SKYFL029 wrong rule, so SKYFL016 stands",
        ),
        (
            "lib/features/y/y_view.dart",
            "// skies-ignore-file: SKYFL001 a platform view hosts no ViewModel\nclass YView {}",
        ),
    ]);
    let diagnosis = analyze(dir.path()).unwrap();
    let standing: Vec<&str> = diagnosis.findings.iter().map(|f| f.code.as_str()).collect();
    assert_eq!(standing, ["SKYFL016"]);
    let reasons: Vec<(&str, &str)> = diagnosis
        .suppressed
        .iter()
        .map(|s| (s.finding.code.as_str(), s.reason.as_str()))
        .collect();
    assert_eq!(
        reasons,
        [
            ("SKYFL029", "the legacy client rotates on its own until the port"),
            ("SKYFL001", "a platform view hosts no ViewModel"),
        ]
    );
}

#[test]
fn a_skies_ignore_without_a_reason_suppresses_nothing_and_says_so() {
    let dir = project(&[(
        HELPER,
        "Future<void> f() => refreshSession(); // skies-ignore: SKYFL029",
    )]);
    let diagnosis = analyze(dir.path()).unwrap();
    assert!(diagnosis.suppressed.is_empty());
    assert_eq!(diagnosis.findings.len(), 1);
    assert!(
        diagnosis.findings[0]
            .message
            .ends_with("(the skies-ignore on line 1 names no reason, so it does not suppress)")
    );
}
