# Skies — Flutter conventions

Flutter is Skies' mobile body, and a supported web body too. React is the other web body
([FRONTEND-CONVENTIONS.md](FRONTEND-CONVENTIONS.md)); both carry the same product guarantees. A product that wants
the same components on every surface builds its phone and web apps from one Flutter codebase, sharing screens, kit,
and ViewModels. A Flutter mobile app beside a React web app gets parity of capability and enforcement, not a literal
translation of React APIs.

1. **Stranger-maintainable.** A Flutter developer unfamiliar with Skies reads normal Widgets, `ChangeNotifier`, Dio,
   `Form`, ARB catalogs, `flutter_test`, and `integration_test`.
2. **Doctor-removable.** Generated wire is committed and behavior is hand-owned: no runtime discovery, source
   generation of behavior, custom widget language, or required Skies base class. Without `skies`, the app still
   builds and runs.

## Opinionated stack

- Flutter stable with strict Dart analysis.
- MVVM: one `ChangeNotifier` ViewModel per screen owns UI state and commands; the View renders.
- OpenAPI Generator [`dart-dio`](https://openapi-generator.tech/docs/generators/dart-dio/) with `built_value`, pinned
  by `skies g client`; Dio is the transport.
- Flutter `Form`/`TextFormField`; `submitOrReveal` forces the invalid path.
- Flutter `gen_l10n` from ARB catalogs assembled from co-located feature catalogs (`skies i18n`).
- `flutter_test` for headless spec cases and [`integration_test`](https://docs.flutter.dev/testing/integration-tests)
  for on-device ones; every case lives in a spec (`SKYFL036`).

This follows Flutter's [architecture guide](https://docs.flutter.dev/app-architecture/guide): View and ViewModel
paired, UI state and commands in the ViewModel, dependencies through constructors.

## One feature, one shape

Initialize from Flutter itself, then add the Skies spine (the scaffold adds `l10n.yaml` and the client/session seams):

```bash
flutter create my_app
skies g flutter-app my_app --path my_app
cd my_app && flutter pub add skies_flutter
skies g feature Profile
```

```text
lib/features/<audience>/<feature>/
  <feature>_view.dart
  <feature>_view_model.dart
lib/l10n/features/
  <feature>_{pt_BR,en,es}.arb
```

`<audience>` mirrors how the product is experienced, never the backend module tree.

- A View observes exactly one ViewModel, imports no Dio/client behavior, and never navigates after paint because of
  state.
- A ViewModel extends `ChangeNotifier`, exposes `AsyncState<T>`, owns commands, and imports neither Widgets nor device
  plugins; platform capabilities enter as constructor ports.
- A composition root supplies generated `dart-dio` operations to the ViewModel. Contract types cross freely;
  operation execution has one data door.
- The View routes loading, failure, empty, and ready through `ResourceBuilder`. No boolean state soup.

`ChangeNotifier` is an ecosystem anchor, not a Skies base class; an app may replace it and keep the contract.

## Runtime correspondence

| Product guarantee | React spelling | Flutter spelling |
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

`skies g client` reads the JSON OpenAPI, drops `Asset`, `Webhook`, `Internal`, and non-app operations, prunes
unreachable components, runs stock OpenAPI Generator, `build_runner`, formatting, and analysis in scratch, then
swaps in only a directory carrying its marker, so a failed run never leaves a half-written client. Parsing, models,
serialization, and HTTP stay OpenAPI Generator/Dio responsibilities.

The contract defaults to the one `Skies.toml` assigns to the package, the output to `packages/<backend>_api`.
`--input <openapi.json>`, `--output <dir>`, `--name <dart_package>`, and `--version <semver>` override them (relative
to the current directory), e.g. `skies g client --package . --input schema/Accounts.json --output
packages/accounts_api --name accounts_api` in a `generate:api` script. With `--input` or `--output`, only the package
is generated: the app already owns its seams and pubspec entry.

The hand-owned client takes its base URL and auth ports from the composition root, replays once through the
single-flight refresh after a non-auth 401, unwraps `Response<T>`, and maps the generated `ErrorBody` to
`SkiesApiException<ErrorBody>`. Transport failures keep their Dio cause.

## Session and routing

Session restoration and 401 replay share the `SessionSeam.bootstrapSession` single-flight door. The refresh token
sits behind an injected secure-storage port (the device keystore on mobile). Sign-in and sign-out run the total
identity reset; rotation runs only the light session reset, so one user's cache never bleeds into the next.

Guards branch on `SessionState`: loading waits, allowed renders, rejected redirects. Authenticated, anonymous, and
capability routes use the same `guardSession`. Routes normalize required params through `requiredParam`, redirect
declaratively, use typed route values, allowlist URL-derived destinations, and call `safeBack` instead of popping an
empty stack.

## Forms, mutations, and feedback

The ViewModel calls `submitOrReveal` with validation, the ordered invalid-field inventory, a valid command, and an
invalid surface. The app-owned `AppInput` exposes `validator`/`errorText`, so field errors show where the control is.

Every write crosses one configured `MutationBoundary`: success invalidates app-owned cached reads and posts success
feedback unless silent; failure always posts error feedback and rethrows for an optional inline surface.
`expectedFailure: true` suppresses the global error note only when the ViewModel models that failure as visible local
state. A success handler that only reloads duplicates the boundary and is warned.

## Pagination

`Page<T>` mirrors the backend's four-member page; `toPageInfo` owns render arithmetic. `Pager` owns the numbered page,
page size, and trimmed debounced search without fetching. `AccumulatedPages` owns load-more state: it replaces page
one, dedupes later pages by stable key, and lets the fresh copy win. ViewModels remain the only request owners.

## Localization and stable error codes

Feature scaffolding emits equal-key ARB catalogs for pt-BR, English, and Spanish. `skies i18n` merges them into the
`app_<locale>.arb` inputs of `gen_l10n`, refusing duplicate keys and locale gaps (`SKYFL011`). `apiErrorCode` /
`apiErrorCopy` resolve a stable `ErrorBody.code` to catalog copy, with a generic fallback.

## Specs and E2E

Flutter features are accepted like every Skies feature: a `.specs/` folder with failure modes, cases in `e2e/`, and a
receipt from `skies proof record` (see [CONVENTIONS.md](CONVENTIONS.md#specs-and-proofs)). The engine is
`flutter_test` and `integration_test` (or Maestro); name each case after its mode
(`testWidgets('FM-2: an expired session lands on sign-in', …)`).

**Every test lives in a spec** (`SKYFL036`): a package's `test/` and `integration_test/` hold no cases, and an
isolated unit gets its own spec. A Dart case imports `package:<app>/...`, which resolves only inside the package, so
the runner **copies** the spec's `e2e/` into a hidden folder of the package per run; `skies g flutter-app` adds
`.skies_spec/` to `.gitignore`, and the doctor never walks hidden folders.

`flutter test` has no JUnit reporter, so the runner writes Dart's JSON report and converts it with
[`junitreport`](https://pub.dev/packages/junitreport). For a package at `app/`, headless cases:

```toml
[runners.flutter]
setup = "flutter pub global activate junitreport"
command = "rm -rf app/test/.skies_spec && mkdir -p app/test/.skies_spec && cp -R {dir}/. app/test/.skies_spec/ && cd app && flutter test test/.skies_spec --file-reporter json:.dart_tool/skies_spec.json; flutter pub global run junitreport:tojunit --input .dart_tool/skies_spec.json --output {report}"
```

The `;` before the conversion is deliberate: a red run fails its tests and must still write its report. For
on-device cases (`IntegrationTestWidgetsFlutterBinding`), copy into `app/integration_test/.skies_spec/` and pass the
device (`flutter test integration_test/.skies_spec -d <device> …`). Red runs in a fresh git worktree, where
`flutter test` resolves the package on its own.

`SKIES_EVIDENCE` and `SKIES_SPEC` reach the runner's host process, not the device: a host-side test reads
`Platform.environment['SKIES_EVIDENCE']`, and an on-device run saves artifacts from its host driver
(`integration_test_driver`'s `responseDataCallback`). An Assay-decided mode carries `[avp: <criterion-id>]` and its
verdict goes to `$SKIES_EVIDENCE/avp-FM-<n>.json`; the tag is optional.

Styling, the widget kit, tokens, and layout are the application's.

## Accessibility — the a11y floor

Accessibility is a floor, on by default, like the .NET CA* security floor and the web's jsx-a11y floor. The native
doctor reads four shapes a screen reader cannot recover from (`SKYFL037`–`040`, in the catalog below). A
`Semantics(label: …)` or `Tooltip` around the widget, or a label inside it (`Icon(semanticLabel:)`), satisfies each.
The rules are static and conservative: a widget passed as a variable, a decoration built by a helper, or an image
handed to a custom widget's slot is never guessed at. They skip tests and generated code (`*.g.dart`,
`*.freezed.dart`, `lib/l10n/`, a generated `packages/<x>_api/`). `IconButton` and image findings are errors (the fix
is one local argument); tap targets and text fields are warnings (a design-system wrapper or `InputDecorationTheme`
may supply the name out of sight). Relax in code, reviewably: `excludeFromSemantics: true` marks an image decorative,
`Semantics(label:)` names a control labelled elsewhere. Flutter's `meetsGuideline` matchers remain a good spec check
for what no static rule can see (contrast, tap-target size).

## Flutter doctor rule catalog

Numbers up to `SKYFL036` keep the `SKYFE` slot of the same concern; gaps are removed rules. `SKYFL009` has no live
React twin: a Flutter ViewModel still must not reach device plugins. `SKYFL037`–`040` are Flutter-only (the web's
floor is jsx-a11y). `SKYFL028`, `031`, `032`, `039`, and `040` are warnings; every other finding is an error.

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
| `SKYFL037` | An `IconButton` (or `.filled`/`.filledTonal`/`.outlined`) carries a `tooltip:` (its label), unless a `Semantics(label:)`/`Tooltip` names it from inside or around. |
| `SKYFL038` | An `Image`/`Image.asset\|network\|file\|memory` has a `semanticLabel:` (a `SvgPicture.*` a `semanticsLabel:`) or `excludeFromSemantics: true`, unless labelled or excluded around it; checked where it renders as written, not when handed to a custom widget's slot. |
| `SKYFL039` | A `GestureDetector`/`InkWell` with `onTap:` whose child tree is only icons, images, and layout boxes carries a label (`Semantics(label:)`, `Tooltip`, or a labelled icon); a child it cannot see is not flagged. |
| `SKYFL040` | A `TextField`/`TextFormField` has a decoration with `labelText`, `label`, or `hintText`; a decoration built elsewhere is not flagged. |

`skies doctor` runs these natively over every Flutter package in `Skies.toml` (`lib/`, `test/`, `integration_test/`,
skipping hidden folders); `skies doctor --package .` runs one package, which is what a package's `lint` script calls.

## Generate versus scaffold

Only contract wire is regenerated (`skies g client`). ViewModels, Views, ARB catalogs, and the client/session/mutation
seams are scaffolded once and owned by the application. There is no Skies widget runtime, MVVM base class, router
adapter, service locator, or styling DSL.
