//! Regression cases from calibrating the SKYFL rules on real Flutter packages
//! (docs/decisions/skies-5-rule-calibration.md): each false-positive class a real app produced stays quiet here, next
//! to the violation the rule must still catch.

use super::diagnose;

fn codes(files: &[(&str, &str)]) -> Vec<String> {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("lib")).unwrap();
    for (path, source) in files {
        let path = dir.path().join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, format!("{source}\n")).unwrap();
    }
    diagnose(dir.path()).unwrap().into_iter().map(|f| f.code).collect()
}

#[test]
fn a_generated_client_package_is_not_app_code() {
    // built_value DTOs named `*_view.dart` are models, not Views: the marker skips the package whole.
    let files = [
        (".skies-generated-client", ""),
        (
            "lib/src/model/charge_view.dart",
            "abstract class ChargeView implements Built<ChargeView, ChargeViewBuilder> {}",
        ),
    ];
    assert!(codes(&files).is_empty());
    // Without the marker the same file is a View missing its ViewModel.
    assert!(codes(&files[1..]).contains(&"SKYFL001".to_string()));
}

#[test]
fn test_harnesses_and_test_routes_are_not_the_shipped_app() {
    let found = codes(&[
        (
            "integration_test/support/native_app_harness.dart",
            "Future<void> seed() => client.api.getAccountApi().register(body);",
        ),
        (
            "integration_test/property_flows_test.dart",
            "import 'package:integration_test/integration_test.dart';\nfinal r = GoRoute(builder: (c, state) => Chat(id: state.uri.queryParameters['chatId']!));",
        ),
        (
            "test/support/mocks.dart",
            "import 'package:mocktail/mocktail.dart';\n// TODO: fake the clock",
        ),
    ]);
    assert!(
        !found
            .iter()
            .any(|c| ["SKYFL002", "SKYFL003", "SKYFL018", "SKYFL023"].contains(&c.as_str())),
        "test code tripped an architecture rule: {found:?}"
    );
    // The same operation call in production code is still off the data door.
    let production = codes(&[(
        "lib/support/seed.dart",
        "Future<void> seed() => client.api.getAccountApi().register(body);",
    )]);
    assert!(production.contains(&"SKYFL002".to_string()));
}

#[test]
fn a_part_shares_its_library_role() {
    let model = "lib/features/home/home_view_model.dart";
    let found = codes(&[
        (
            model,
            "part 'home_requests.dart';\nfinal class HomeViewModel { AsyncState<int> state = const AsyncLoading<int>(); }",
        ),
        (
            "lib/features/home/home_requests.dart",
            "part of 'home_view_model.dart';\nextension on HomeViewModel { Future<void> load() => client.api.getHomeApi().listCards(); }",
        ),
    ]);
    assert!(
        !found.contains(&"SKYFL002".to_string()),
        "a ViewModel part may call the client: {found:?}"
    );
    let view_part = codes(&[(
        "lib/features/home/home_header.dart",
        "part of 'home_view.dart';\nWidget header() => Text('Hardcoded');",
    )]);
    assert!(
        view_part.contains(&"SKYFL014".to_string()),
        "a View part holds copy too: {view_part:?}"
    );
    assert!(
        !view_part.contains(&"SKYFL001".to_string()),
        "a part is not a View missing its ViewModel"
    );
}

#[test]
fn guarded_pops_and_overlay_closes_are_not_bare_backs() {
    let quiet: &[&str] = &[
        // The routing seam's guarded helper.
        "void popOrGo(BuildContext context, String fallback) { if (context.canPop()) { context.pop(); } else { context.go(fallback); } }",
        // A pop that hands a result back to the route that pushed it.
        "void done(BuildContext context, String code) => Navigator.of(context).pop(code);",
        "void confirm(BuildContext context) => Navigator.pop(context, true);",
        // A pop inside the builder of an overlay this code opened.
        "void ask(BuildContext context) { showDialog(context: context, builder: (ctx) => TextButton(onPressed: () => Navigator.of(ctx).pop(), child: const SizedBox())); }",
        // A page this file pushes imperatively (a scanner) closes itself.
        "Future<String?> open(BuildContext context) => Navigator.of(context).push(MaterialPageRoute(builder: (_) => const Scanner()));\nWidget cancel(BuildContext context) => TextButton(onPressed: () => Navigator.of(context).pop(), child: const SizedBox());",
        // An overlay's own route context, closed from outside its builder.
        "void open(BuildContext context) { showAppModal(context, (routeContext) { saved = routeContext; }); }\nvoid close() { Navigator.of(saved).pop(); }",
    ];
    for source in quiet {
        let found = codes(&[("lib/features/x/helper.dart", source)]);
        assert!(!found.contains(&"SKYFL019".to_string()), "{source} → {found:?}");
    }
}

#[test]
fn an_unguarded_back_in_an_arrow_body_is_still_a_bare_back() {
    // The grammar reads `() => Navigator.of(context).pop()` with the arrow inside the receiver; normalized, it is caught.
    let arrow = "Widget build(BuildContext context) => TextButton(onPressed: () => Navigator.of(context).pop(), child: const SizedBox());";
    assert!(codes(&[("lib/features/x/page.dart", arrow)]).contains(&"SKYFL019".to_string()));
    let block =
        "Widget build(BuildContext context) => TextButton(onPressed: () { context.pop(); }, child: const SizedBox());";
    assert!(codes(&[("lib/features/x/page.dart", block)]).contains(&"SKYFL019".to_string()));
}
