//! Rule contract: every kept SKYFL rule fires on its violation, stays quiet on the conformant shape, and ignores
//! lookalikes in comments and strings that the 4.x regexes tripped on.

use std::path::Path;

use super::diagnose;
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

const MODEL: &str = "lib/features/x/x_view_model.dart";
const VIEW: &str = "lib/features/x/x_view.dart";
const STATEFUL_MODEL: &str = "final class XViewModel { AsyncState<int> state = const AsyncLoading<int>(); }";

#[test]
fn every_kept_rule_fires_on_its_violation() {
    let cases: &[(&str, &[(&str, &str)])] = &[
        ("SKYFL001", &[(VIEW, "import 'package:dio/dio.dart';")]),
        (
            "SKYFL002",
            &[(
                "lib/helper.dart",
                "void f() => client.api.getWalletApi().listWallets();",
            )],
        ),
        (
            "SKYFL003",
            &[("lib/helper.dart", "import 'package:mocktail/mocktail.dart';")],
        ),
        ("SKYFL004", &[(MODEL, "Widget build(BuildContext context) => value;")]),
        (
            "SKYFL007",
            &[(MODEL, "final class XViewModel extends ChangeNotifier {}")],
        ),
        ("SKYFL010", &[(VIEW, "class XView {}"), (MODEL, STATEFUL_MODEL)]),
        (
            "SKYFL011",
            &[
                ("lib/l10n/x_en.arb", r#"{"title":"Title","empty":"Empty"}"#),
                ("lib/l10n/x_pt_BR.arb", r#"{"title":"Título"}"#),
            ],
        ),
        (
            "SKYFL013",
            &[(
                MODEL,
                "AsyncState<int> state; Future<void> save() async { await write(); }",
            )],
        ),
        (
            "SKYFL014",
            &[(VIEW, "Widget build(BuildContext c) => Text('Hardcoded');")],
        ),
        (
            "SKYFL015",
            &[(VIEW, "void f() { addPostFrameCallback((_) { context.go('/home'); }); }")],
        ),
        (
            "SKYFL016",
            &[("lib/features/x/helper.dart", "void f() => setAccessToken('x');")],
        ),
        (
            "SKYFL017",
            &[(
                "lib/routes/account_guard.dart",
                "void f() { if (!isAuthenticated) redirect(); }",
            )],
        ),
        (
            "SKYFL018",
            &[(
                "lib/routes/detail_route.dart",
                "void f() { final id = state.pathParameters['id']; }",
            )],
        ),
        ("SKYFL019", &[("lib/helper.dart", "void f() => context.pop();")]),
        (
            "SKYFL020",
            &[(
                "lib/client.dart",
                "final dio = Dio(BaseOptions(baseUrl: 'http://localhost:8080'));",
            )],
        ),
        (
            "SKYFL021",
            &[("lib/features/x/helper.dart", "Widget build() => Html(data: body);")],
        ),
        (
            "SKYFL022",
            &[("lib/routes/login_route.dart", "void f() => context.go(returnTo);")],
        ),
        ("SKYFL023", &[("lib/helper.dart", "// TODO wire later")]),
        (
            "SKYFL027",
            &[(
                MODEL,
                "AsyncState<int> state; Future<void> save() async { try { await write(); } catch (error) { state = AsyncFailure(error, StackTrace.current); } }",
            )],
        ),
        (
            "SKYFL028",
            &[("lib/helper.dart", "final onSuccess = () { refetch(); };")],
        ),
        (
            "SKYFL029",
            &[("lib/features/x/helper.dart", "Future<void> f() => refreshSession();")],
        ),
        (
            "SKYFL030",
            &[(
                "lib/routes/detail_route.dart",
                "void f() => context.go(route as dynamic);",
            )],
        ),
        (
            "SKYFL031",
            &[(MODEL, "AsyncState<int> state; void submit() { form.validate(); }")],
        ),
        (
            "SKYFL032",
            &[("lib/ui/app_input.dart", "Widget build() => TextFormField();")],
        ),
    ];
    for (code, files) in cases {
        let found = codes(files);
        assert!(found.iter().any(|c| c == code), "{code} was absent; got {found:?}");
    }
}

#[test]
fn a_scaffolded_feature_is_clean() {
    let feature = crate::flutter::feature::render_feature("wallets").unwrap();
    let mut files: Vec<(String, String)> = Vec::new();
    for (name, source) in &feature.lib {
        files.push((format!("lib/features/wallets/{name}"), source.clone()));
    }
    for (name, source) in &feature.l10n {
        files.push((format!("lib/l10n/features/{name}"), source.clone()));
    }
    let borrowed: Vec<(&str, &str)> = files.iter().map(|(p, s)| (p.as_str(), s.as_str())).collect();
    assert_eq!(codes(&borrowed), Vec::<String>::new());
}

#[test]
fn a_real_view_reports_architecture_and_state_gaps_by_their_ids() {
    let found = codes(&[
        (
            "lib/features/wallets/wallets_view.dart",
            "import 'package:dio/dio.dart';\nWidget f() => Text('hardcoded');",
        ),
        (
            "lib/features/wallets/wallets_view_model.dart",
            "final class WalletsViewModel {}",
        ),
    ]);
    for expected in ["SKYFL001", "SKYFL007", "SKYFL014"] {
        assert!(found.iter().any(|c| c == expected), "{expected} missing from {found:?}");
    }
}

#[test]
fn retired_proof_and_design_rules_never_fire() {
    let found = codes(&[
        (
            "lib/features/x/x_view.dart",
            "Widget f() => Card(child: Row(style: s, padding: EdgeInsets.all(13)));",
        ),
        (
            "lib/features/x/x_view_model.dart",
            "import 'package:camera/camera.dart';\n/// @verify works\n/// @e2e x-happy\nAsyncState<int> state;",
        ),
        ("lib/helper.dart", "const c = Color(0xff112233); const h = '#abcdef';"),
        (
            "test/helper_test.dart",
            "void main() { test('later', () {}, skip: true); }",
        ),
    ]);
    for retired in [
        "SKYFL005", "SKYFL006", "SKYFL008", "SKYFL009", "SKYFL012", "SKYFL024", "SKYFL025", "SKYFL026", "SKYFL033",
        "SKYFL034", "SKYFL035",
    ] {
        assert!(!found.iter().any(|c| c == retired), "{retired} still fires: {found:?}");
    }
}

#[test]
fn comments_and_strings_do_not_trigger_code_rules() {
    let found = codes(&[
        (
            VIEW,
            "// Text('in a comment') uses package:dio/dio.dart\nfinal note = \"call context.pop() and setAccessToken\";",
        ),
        (MODEL, STATEFUL_MODEL),
    ]);
    assert_eq!(found, ["SKYFL010"], "only the missing ResourceBuilder remains");
}

#[test]
fn seams_are_allowed_what_features_are_not() {
    let found = codes(&[
        (
            "lib/session.dart",
            "void wire(api) { setAccessToken('t'); refreshSession(); }",
        ),
        (
            "lib/skies_client.dart",
            "Future<T> r<T>(op) => executeSkiesRequest<T, ErrorBody>(op);",
        ),
        ("lib/html/render.dart", "Widget f() => Html(data: body);"),
    ]);
    assert_eq!(found, Vec::<String>::new());
}

#[test]
fn expected_mutation_failures_require_a_modeled_local_surface() {
    let dir = project(&[(
        "lib/features/sign_in/sign_in_view_model.dart",
        "final class SignInViewModel { AsyncState<int> s; Future<void> submit() => boundary.run(() async {}, successMessage: 'signed in', expectedFailure: true); }",
    )]);
    let findings = diagnose(dir.path()).unwrap();
    assert!(
        findings
            .iter()
            .any(|f| f.code == "SKYFL013" && f.message.contains("expected failure"))
    );
}

#[test]
fn guarded_validation_and_route_params_pass() {
    let found = codes(&[
        (
            MODEL,
            "AsyncState<int> state; void submit() { if (!form.validate()) return; send(); }",
        ),
        (
            "lib/routes/detail_route.dart",
            "void f() { final id = requiredParam(state.pathParameters['id']); }",
        ),
    ]);
    assert_eq!(found, Vec::<String>::new());
}

#[test]
fn warnings_are_warnings_and_findings_carry_lines() {
    let dir = project(&[("lib/helper.dart", "void a() {}\nfinal onSuccess = () { refetch(); };")]);
    let findings = diagnose(dir.path()).unwrap();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert_eq!(findings[0].line, Some(2));
    assert!(findings[0].file.ends_with(Path::new("lib/helper.dart")));
}

#[test]
fn a_package_without_lib_cannot_be_diagnosed() {
    let dir = tempfile::tempdir().unwrap();
    assert!(diagnose(dir.path()).is_err());
}

#[test]
fn a_test_outside_a_spec_is_flagged_once_per_file() {
    let dir = project(&[
        (
            "test/widget_test.dart",
            "import 'package:flutter_test/flutter_test.dart';\nvoid main() {\n  group('g', () {\n    testWidgets('a', (t) async {});\n    test('b', () {});\n  });\n}",
        ),
        (
            "integration_test/app_test.dart",
            "import 'package:integration_test/integration_test.dart';\nimport 'package:flutter_test/flutter_test.dart';\nvoid main() { testWidgets('a', (t) async {}); }",
        ),
        (
            "test/unit_test.dart",
            "import 'package:test/test.dart';\nvoid main() { test('a', () {}); }",
        ),
    ]);
    let findings: Vec<_> = diagnose(dir.path())
        .unwrap()
        .into_iter()
        .filter(|f| f.code == "SKYFL036")
        .collect();
    assert_eq!(findings.len(), 3, "{findings:?}");
    let widget = findings
        .iter()
        .find(|f| f.file.ends_with(Path::new("test/widget_test.dart")))
        .unwrap();
    assert_eq!(widget.line, Some(3));
    assert_eq!(widget.severity, Severity::Error);
}

#[test]
fn spec_cases_copied_into_a_hidden_folder_and_lookalikes_are_not_tests() {
    let found = codes(&[
        // Where the Flutter runner copies a spec's cases: hidden, so the doctor never walks it.
        (
            "integration_test/.skies_spec/deposit_test.dart",
            "import 'package:flutter_test/flutter_test.dart';\nvoid main() { testWidgets('FM-1: x', (t) async {}); }",
        ),
        // No runner import: an app function that happens to be called `test` is not a test.
        ("lib/helper.dart", "bool test(int v) => v > 0;\nfinal ok = test(1);"),
        // A method named `test` on a receiver is not the runner's top-level function.
        (
            "test/regex_test.dart",
            "import 'package:test/test.dart';\nfinal ok = RegExp('a').hasMatch('a') && matcher.test('a');",
        ),
    ]);
    assert!(!found.iter().any(|c| c == "SKYFL036"), "{found:?}");
}
