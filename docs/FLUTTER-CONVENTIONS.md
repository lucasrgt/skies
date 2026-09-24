# Skies — Flutter conventions

Flutter is Skies' mobile body, and a supported web body too. React is the other web body
([FRONTEND-CONVENTIONS.md](FRONTEND-CONVENTIONS.md)); both carry the same product guarantees. A product that
wants the same components on every surface builds its phone apps and its web app from one Flutter codebase, so
the screens, the kit, and the ViewModels are shared rather than mirrored. A product that pairs a Flutter mobile app
with a React web app gets parity of capability and enforcement, not a literal translation of React APIs. Output
remains plain, idiomatic Dart and Flutter; deleting the `skies` CLI leaves an ordinary application that still
builds and runs.

The two laws remain absolute:

1. **Stranger-maintainable.** A Flutter developer unfamiliar with Skies reads normal Widgets, `ChangeNotifier`,
   Dio, `Form`, ARB catalogs, `flutter_test`, and `integration_test`.
2. **Doctor-removable.** Generated wire is committed; application behavior is hand-owned; no runtime discovery,
   source generation of behavior, custom widget language, or required Skies base class exists.

## Opinionated stack

- Flutter stable with strict Dart analysis.
- MVVM using one `ChangeNotifier` ViewModel per screen. The ViewModel owns UI state and commands; the View renders.
- OpenAPI Generator [`dart-dio`](https://openapi-generator.tech/docs/generators/dart-dio/), pinned by `skies g client`,
  using `built_value`; Dio is the transport.
- Flutter `Form`/`TextFormField`; `submitOrReveal` forces the invalid path.
- Flutter `gen_l10n` from ARB catalogs assembled from co-located feature catalogs (`skies i18n`).
- Flutter's official [`integration_test`](https://docs.flutter.dev/testing/integration-tests) for on-device spec
  cases and `flutter_test` for headless ones. Every case lives in a spec (`SKYFL036`); an isolated unit gets its own
  spec, with its failure modes written first.

This follows Flutter's [application architecture guide](https://docs.flutter.dev/app-architecture/guide): View and
ViewModel are paired, UI state and commands live in the ViewModel, and dependencies enter through constructors.

## One feature, one shape

Initialize from Flutter itself, then add the Skies spine:

```bash
flutter create my_app
skies g flutter-app my_app --path my_app
cd my_app && flutter pub add skies_flutter
skies g feature Profile
```

The app scaffold adds `l10n.yaml` and the client/session seams. It does not replace Flutter's project generator.

```text
lib/features/<audience>/<feature>/
  <feature>_view.dart
  <feature>_view_model.dart
lib/l10n/features/
  <feature>_{pt_BR,en,es}.arb
```

`<audience>` mirrors how the product is experienced, never the backend module tree.

- A View observes exactly one ViewModel. It imports no Dio/client behavior and performs no navigation caused by
  state after paint.
- A ViewModel extends `ChangeNotifier`, exposes `AsyncState<T>`, owns commands, and imports neither Widgets nor
  device plugins. Platform capabilities enter as constructor ports.
- A composition root supplies generated `dart-dio` operations to the ViewModel. Contract types remain free to
  cross the boundary; operation execution has one data door.
- The View routes loading, failure, empty, and ready through `ResourceBuilder`. No boolean state soup.

`ChangeNotifier` is an ecosystem anchor, not a Skies base class. The application may replace notification while
retaining the View/ViewModel contract; no behavior depends on the doctor.

## Runtime correspondence

| Product guarantee | React spelling | Idiomatic Flutter spelling |
|---|---|---|
| Closed server state | `AsyncState`, `toAsyncState`, `combineAsyncStates` | sealed `AsyncState`, `toAsyncState`, typed `combineAsyncStates2/3` |
| Exhaustive rendering | `<Resource>` | `ResourceBuilder` |
| Session read state | `SessionState`, `toSessionState` | sealed `SessionState`, `toSessionState` |
| Session write door | `createSessionSeam` | `SessionSeam` |
| Cold-start gate | `useSession` | `SessionBootstrap` controller |
| Rotation collapse | `singleFlight` | `SingleFlight<T>` |
| Symmetric route guard | `guardSession` | sealed `SessionAccess` + `guardSession` |
| Deep-link-safe back | `safeBack` | typed `BackRouter` + `safeBack` |
| Required route param | `requiredParam` | sealed `RequiredParam` + `requiredParam` |
| Localized API error | `apiErrorCode`/`apiErrorCopy` | generated-body reader + `apiErrorCode`/`apiErrorCopy` |
| Page rendering facts | `Page`, `toPageInfo` | `Page<T>`, `toPageInfo` |
| Numbered search pager | `usePager` | disposable `Pager` controller |
| Load-more fold | `useAccumulatedPages` | `AccumulatedPages<T,K>` controller |
| Visible invalid submit | `submitOrReveal` over RHF | `submitOrReveal` over Flutter `Form` facts |
| Global write defaults | `MutationCache` + feedback seam | injected `MutationBoundary` + `FeedbackSink` |

## Generated wire and client seam

```text
packages/client.gen/<api>/  generated Dart package; replaced atomically
lib/skies_client.dart       hand-owned base URL, auth, and ErrorBody seam
lib/session.dart            hand-owned SessionSeam composition
lib/mutations.dart          hand-owned MutationBoundary composition
```

`skies g client` reads JSON OpenAPI, excludes `Asset`, `Webhook`, `Internal`, and explicitly non-app operations, prunes
unreachable components, invokes stock OpenAPI Generator, runs `build_runner`, formatting, and analysis, stamps the
source contract, then atomically replaces only a marked generated directory. Parsing, models, serialization, and
HTTP behavior remain OpenAPI Generator/Dio responsibilities.

By default the contract is the one `Skies.toml` assigns to the package and the output is `packages/<backend>_api`.
`--input <openapi.json>`, `--output <dir>`, `--name <dart_package>`, and `--version <semver>` override those defaults
(relative paths resolve against the current directory), so a package with its own schema can keep
`skies g client --package . --input schema/Accounts.json --output packages/accounts_api --name accounts_api` in its
`generate:api` script. With `--input` or `--output`, only the package is generated: the app already owns its seams
and its pubspec entry.

The hand-owned client accepts its base URL and auth ports from the composition root, performs one single-flight
refresh replay after a non-auth 401, unwraps `Response<T>`, and maps the canonical generated `ErrorBody` to
`SkiesApiException<ErrorBody>`. Transport failures retain their original Dio cause.

Every app-facing OpenAPI `operationId` must be consumed from a ViewModel or sanctioned session/guard/client seam.
Endpoint ownership comes from the application's `contract/*.json` files or an explicit `--contract`.
An E2E flow's `backendContract` may reference a broader backend schema: it validates observed operations and
consumed feature coverage without assigning unrelated endpoints to that application.
`skies-flutter-endpoint-coverage` is warning-tier while building and blocking under `--strict`. Contract freshness
is blocking after the first generated stamp.

## Session and routing

Session restoration and 401 replay call the same `SessionSeam.bootstrapSession` single-flight door. The refresh
token lives behind an injected secure-storage port (the device keystore on mobile). Explicit sign-in and sign-out
run the total identity reset; rotation runs only the light session reset. This holds the invariant the React seam
holds too: one user's cache can never bleed into the next identity.

Guards branch on `SessionState`: loading waits, allowed renders, rejected redirects. Authenticated, anonymous, and
capability routes use the same `guardSession` primitive. Routes normalize required params through `requiredParam`,
redirect declaratively, use typed route values, allowlist URL-derived destinations, and call `safeBack` rather than
blindly popping an empty stack.

## Forms, mutations, and feedback

The ViewModel owns form logic and calls `submitOrReveal` with validation, the ordered invalid-field inventory, a
valid command, and an invalid surface. The app-owned `AppInput` exposes `validator`/`errorText`, so field errors are
visible where the control lives.

Every write crosses a single configured `MutationBoundary`. Success invalidates app-owned cached reads and posts
success feedback unless explicitly silent. Failure always posts error feedback and rethrows for an optional richer
inline surface. `expectedFailure: true` suppresses the global error note only when the ViewModel models that failure
as a visible local state. Manual success handlers whose only work is reloading duplicate the boundary and are warned.

## Pagination

`Page<T>` mirrors the backend's four-member page. `toPageInfo` owns render arithmetic. `Pager` owns numbered page,
page size, and trimmed debounced search without fetching. `AccumulatedPages` owns load-more state, replaces page one,
deduplicates later pages by stable key, and lets the fresh copy win when boundaries move. ViewModels remain the only
request owners.

## Localization and stable error codes

Feature scaffolding emits equal-key ARB catalogs for pt-BR, English, and Spanish. `skies-flutter-i18n assemble`
merges them into the conventional `app_<locale>.arb` inputs for `gen_l10n`; duplicate keys fail. `check` enforces
locale parity. Error-code coverage derives the closed `ErrorBody.code` enum from OpenAPI and requires an
`apiError_<code>` catalog key; together, stable backend code reaches localized copy in every language.

## Specs and E2E

Flutter features are accepted like every other Skies feature: a spec folder under `.specs/` with its failure modes,
cases in `e2e/`, and a receipt from `skies proof record` (see
[CONVENTIONS.md](CONVENTIONS.md#specs-and-proofs)). The engine is Flutter's own `flutter_test` and
`integration_test` (or Maestro). Declare a runner in `Skies.toml` that runs one spec folder and writes a JUnit report,
and name each case after the failure mode it covers (`testWidgets('FM-2: an expired session lands on sign-in', …)`).

**Every test lives in a spec** (`SKYFL036`): a package's own `test/` and `integration_test/` hold no cases, and an
isolated unit (a ViewModel, a formatter) gets its own spec. A Dart case imports the app as `package:<app>/...`,
which only resolves inside the package, so the spec's `e2e/` stays at the repository root and the runner **copies**
it into a hidden folder of the package before running it. The copy is regenerated per run; `skies g flutter-app`
adds `.skies_spec/` to the package's `.gitignore`, and the doctor never walks hidden folders.

`flutter test` has no JUnit reporter, so the runner writes Dart's JSON report (`--file-reporter json:<path>`) and
converts it with [`junitreport`](https://pub.dev/packages/junitreport). For a package at `app/`, headless cases
(`test`, `testWidgets` in the VM, no device):

```toml
[runners.flutter]
setup = "flutter pub global activate junitreport"
command = "rm -rf app/test/.skies_spec && mkdir -p app/test/.skies_spec && cp -R {dir}/. app/test/.skies_spec/ && cd app && flutter test test/.skies_spec --file-reporter json:.dart_tool/skies_spec.json; flutter pub global run junitreport:tojunit --input .dart_tool/skies_spec.json --output {report}"
```

The `;` before the conversion is deliberate: a red run fails its tests, and its report must still be written. For
on-device cases (`IntegrationTestWidgetsFlutterBinding`), copy into `app/integration_test/.skies_spec/` instead and
pass the device (`flutter test integration_test/.skies_spec -d <device> …`). `skies proof record` runs red in a fresh
git worktree; `flutter test` resolves the package there on its own.

Every runner gets `SKIES_EVIDENCE` and `SKIES_SPEC` in its environment. The runner's host process sees them, not
the device: a host-side test reads `Platform.environment['SKIES_EVIDENCE']`, and an on-device run saves artifacts
from its host driver (`integration_test_driver`'s `responseDataCallback`). A failure mode an Assay archetype decides
carries `[avp: <criterion-id>]` on its spec.md line, and its verdict goes to `$SKIES_EVIDENCE/avp-FM-<n>.json`;
the mode then passes only with a passing verdict. The tag is optional.

Styling, the widget kit, tokens, and layout are the application's. Accessibility is too; Flutter's
`meetsGuideline` matchers are a good failure-mode check for a spec, not a framework rule.

## Flutter doctor rule catalog

Every number preserves the corresponding `SKYFE` semantic slot; only the ecosystem spelling changes. `SKYFL009` is
the exception: its React twin kept ViewModels free of React Native and went with that track, while a Flutter
ViewModel still must not reach device plugins. The doctor enforces architecture only.

| Rule | Flutter enforcement |
|---|---|
| `SKYFL001` | View purity: no Dio/client behavior in a View. |
| `SKYFL002` | Generated operations execute only in ViewModels or sanctioned infrastructure doors. |
| `SKYFL003` | No mock/fixture framework in production Dart. |
| `SKYFL004` | ViewModel contains no Widget, `BuildContext`, Material, Cupertino, or navigation API. |
| `SKYFL007` | A server-backed ViewModel exposes closed `AsyncState`. |
| `SKYFL009` | ViewModel imports no device/plugin capability; inject a port. |
| `SKYFL010` | A View renders async state through `ResourceBuilder`. |
| `SKYFL011` | Every ARB locale family has identical keys. |
| `SKYFL013` | Every mutation has global or explicit visible failure handling. |
| `SKYFL014` | User-facing View copy comes from localizations. |
| `SKYFL015` | State-driven redirect is declarative, not a post-frame/listener navigation. |
| `SKYFL016` | Access/refresh tokens are written only through the session/client seam. |
| `SKYFL017` | Guards branch on tri-state session, never `isAuthenticated`. |
| `SKYFL018` | Required route ids pass through `requiredParam`. |
| `SKYFL019` | Back uses `safeBack`, never an unconditional pop. |
| `SKYFL020` | Dio base URL is injected/configured, never a hardcoded host. |
| `SKYFL021` | Raw HTML/WebView rendering exists only in audited `lib/html`. |
| `SKYFL022` | URL-derived navigation targets pass through an allowlist. |
| `SKYFL023` | Production Dart contains no unfinished placeholder. |
| `SKYFL027` | Write features have one configured `MutationBoundary`. |
| `SKYFL028` | Success handlers do not repeat an invalidation-only ritual. |
| `SKYFL029` | Refresh rotation is consumed only by session/client doors. |
| `SKYFL030` | Navigation targets do not escape typed routes through dynamic/Object casts. |
| `SKYFL031` | Form submit carries an explicit invalid path. |
| `SKYFL032` | App form fields surface validator/error state. |
| `SKYFL036` | Tests live in a spec: a top-level `test(`, `testWidgets(`, or `group(` in a file importing `package:test`, `package:flutter_test`, or `package:integration_test` is flagged outside `.specs/` (once per file). |

`skies doctor` runs these rules natively over every Flutter package declared in `Skies.toml` (its `lib/`, `test/`,
and `integration_test/`, skipping hidden folders);
`skies doctor --package .` runs them over one package, which is what a package's own `lint` script calls. `SKYFL028`, `031`,
and `032` are warnings; every other finding is an error. The numbers keep the corresponding `SKYFE` slots, so gaps
are the rules that were removed in Skies 5.

## Generate versus scaffold

Only contract wire is regenerated (`skies g client`). ViewModels, Views, ARB catalogs, and the client/session/mutation
seams are one-shot scaffolded source owned by the application. Nothing re-emits or overwrites behavior.
There is no Skies widget runtime, MVVM base class, router adapter, service locator, or styling DSL.
