# Skies — Flutter conventions and harness

Flutter is a native body for the same product guarantees defined in
[FRONTEND-CONVENTIONS.md](FRONTEND-CONVENTIONS.md). Parity means equivalent capability and enforcement, not a
literal translation of React APIs. Output remains plain, idiomatic Dart and Flutter; deleting the npm tooling and
doctor leaves an ordinary application that still builds and runs.

The two laws remain absolute:

1. **Stranger-maintainable.** A Flutter developer unfamiliar with Skies reads normal Widgets, `ChangeNotifier`,
   Dio, `Form`, ARB catalogs, `flutter_test`, and `integration_test`.
2. **Doctor-removable.** Generated wire is committed; application behavior is hand-owned; no runtime discovery,
   source generation of behavior, custom widget language, or required Skies base class exists.

## Opinionated stack

- Flutter stable with strict Dart analysis.
- MVVM using one `ChangeNotifier` ViewModel per screen. The ViewModel owns UI state and commands; the View renders.
- OpenAPI Generator [`dart-dio`](https://openapi-generator.tech/docs/generators/dart-dio/), pinned by the wrapper,
  using `built_value`; Dio is the transport.
- Flutter `Form`/`TextFormField` through the app-owned UI kit; `submitOrReveal` forces the invalid path.
- Flutter `gen_l10n` from ARB catalogs assembled from co-located feature catalogs.
- `flutter_test` for unit, widget, and semantic AVP proofs; Flutter's official
  [`integration_test`](https://docs.flutter.dev/testing/integration-tests) for real-device journeys.

This follows Flutter's [application architecture guide](https://docs.flutter.dev/app-architecture/guide): View and
ViewModel are paired, UI state and commands live in the ViewModel, and dependencies enter through constructors.

## One feature, one shape

Initialize from Flutter itself, then add the removable Skies harness:

```bash
flutter create my_app
cd my_app
npx --yes --package skies-flutter skies-flutter-app .
npm install
flutter pub add skies_flutter
flutter pub add "dev:integration_test:{sdk: flutter}"
```

The app scaffold adds only `package.json` proof scripts, `l10n.yaml`, and `e2e/flows.json`. It does not replace
Flutter's project generator or rewrite `pubspec.yaml`.

```text
lib/features/<audience>/<feature>/
  <feature>_view.dart
  <feature>_view_model.dart
test/features/<audience>/<feature>/
  <feature>_view_test.dart
  <feature>_view_model_test.dart
  <feature>.assay_test.dart
lib/l10n/features/
  <feature>_{pt_BR,en,es}.arb
integration_test/
  <journey>_test.dart
e2e/flows.json
```

The test tree mirrors `lib/`, which is Dart's conventional separation, while the filenames preserve one semantic
feature unit. `<audience>` mirrors how the product is experienced, never the backend module tree.

- A View observes exactly one ViewModel. It imports no Dio/client behavior and performs no navigation caused by
  state after paint.
- A ViewModel extends `ChangeNotifier`, exposes `AsyncState<T>`, owns commands, and imports neither Widgets nor
  device plugins. Platform capabilities enter as constructor ports.
- A composition root supplies generated `dart-dio` operations to the ViewModel. Contract types remain free to
  cross the boundary; operation execution has one data door.
- The View routes loading, failure, empty, and ready through `ResourceBuilder`. No boolean state soup.
- The app-owned `ui/` kit is the only paint door. Views compose its typed Widgets and pass no free-form `style`.

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
| Real-backend E2E ledger | Playwright backend observer | test-only Dio `BackendLedgerInterceptor` |

The machine-readable correspondence is
[`flutter-sdk/parity/flutter-react.parity.json`](../flutter-sdk/parity/flutter-react.parity.json). Its verifier is a
normal SDK test; a missing source, proof, tool, or rule slot fails the build.

## Generated wire and client seam

```text
packages/client.gen/<api>/  generated Dart package; replaced atomically
lib/skies_client.dart       hand-owned base URL, auth, ErrorBody, and test-interceptor seam
lib/session.dart            hand-owned SessionSeam composition
lib/mutations.dart          hand-owned MutationBoundary composition
```

The wrapper reads JSON OpenAPI, excludes `Asset`, `Webhook`, `Internal`, and explicitly non-app operations, prunes
unreachable components, invokes stock OpenAPI Generator, runs `build_runner`, formatting, and analysis, stamps the
source contract, then atomically replaces only a marked generated directory. Parsing, models, serialization, and
HTTP behavior remain OpenAPI Generator/Dio responsibilities.

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

Session restoration and 401 replay call the same `SessionSeam.bootstrapSession` single-flight door. Native refresh
tokens live behind an injected secure-storage port. Explicit sign-in and sign-out run the total identity reset;
rotation runs only the light session reset. This preserves the React security invariant that one user's cache can
never bleed into the next identity.

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

## Design vocabulary

`skies-flutter-design` scaffolds ordinary Dart into the application:

- the same closed spacing, radius, text-role, shadow, motion, color-role, and breakpoint taxonomy;
- light and dark semantic role maps owned by the app;
- app-owned `AppScreen`, `AppStack`, `AppText`, `AppButton`, `AppField`, `AppInput`, `AppCard`, list, empty, and
  error Widgets;
- minimum 44px actions, disabled/loading behavior, live-region errors, label/error form anatomy, and semantic color
  pairs implemented inside the kit.

Raw material Widgets and free-form styles remain legal inside `ui/`, where the vocabulary is implemented. Feature
Views use the closed kit. Token values and the styling/theme-switch mechanism remain application choices.

## Verification and E2E

Each ViewModel declares `@verify <criterion>` and reciprocal `@e2e <flow-id>` documentation markers. Its mirrored
`*.assay_test.dart` carries an executable `@avp` test for each criterion. These are ordinary `flutter_test` tests;
no custom test engine exists. Each case carries the ordinary `avp` test tag so normal unit/widget runs can exclude
the deliberate-red acceptance partition while `skies gate` executes its exact files.

`e2e/flows.json` binds one visible feature per flow, distinct happy/sad paths, exact criteria/evidence, terminal
Widget keys, consumed backend slices, and the checked-in contract. Each spec is an official Flutter
`integration_test`. When backend slices are named, the test attaches `BackendLedgerInterceptor` to the application's
real Dio instance and calls `expectSlices` with the required success/error outcome after driving the visible UI.
Mocks, skipped tests, metadata-only evidence, and an unobserved typed call do not prove the seam.

Flutter accessibility is an ecosystem-specific mirror of the React Native accessibility plugin. Every generated
widget proof executes Flutter's `androidTapTargetGuideline`, `labeledTapTargetGuideline`, and
`textContrastGuideline`; the unified doctor reports `SKYFL-A11Y` when a View proof omits one. The app-owned kit
provides Semantics, live error regions, labels, disabled/loading behavior, and 44px targets by construction.

The canonical unfiltered runner is `flutter test integration_test`. Backend write slices referenced by Flutter
flows still require co-located .NET happy and sad `[Journey]` proofs through `skies-flutter-journey-parity`.

When a manifest-declared frontend package also has `pubspec.yaml`, the repository `skies gate` selects Flutter
automatically: affected `_test.dart` proofs run through `flutter test`, `*.assay_test.dart` forms the AVP partition,
surface flows run through `integration_test`, strict `SKYFL` replaces the React ESLint leg, and backend roots from
the same `[products.*]` section feed journey parity. React Native packages continue to use their existing
Vitest/Assay/Maestro path.

## Flutter doctor rule catalog

Every number preserves the corresponding `SKYFE` semantic slot; only the ecosystem spelling changes.

| Rule | Flutter enforcement |
|---|---|
| `SKYFL001` | View purity: no Dio/client behavior in a View. |
| `SKYFL002` | Generated operations execute only in ViewModels or sanctioned infrastructure doors. |
| `SKYFL003` | No mock/fixture framework in production Dart. |
| `SKYFL004` | ViewModel contains no Widget, `BuildContext`, Material, Cupertino, or navigation API. |
| `SKYFL005` | Every ViewModel has a mirrored unit test that imports and executes it. |
| `SKYFL006` | Every View has a mirrored `testWidgets` integration proof. |
| `SKYFL007` | A server-backed ViewModel exposes closed `AsyncState`. |
| `SKYFL008` | Every app-facing OpenAPI operation has a legal consumer; strict mode blocks gaps. |
| `SKYFL009` | ViewModel imports no device/plugin capability; inject a port. |
| `SKYFL010` | A View renders async state through `ResourceBuilder`. |
| `SKYFL011` | Every ARB locale family has identical keys. |
| `SKYFL012` | Raw hex exists only in tokens/theme/ui boundaries. |
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
| `SKYFL024` | Views compose the app-owned UI kit, not raw visual Widgets or style arguments. |
| `SKYFL025` | Spacing and typography use the closed scale outside tokens/ui. |
| `SKYFL026` | Colors use semantic roles outside tokens/ui. |
| `SKYFL027` | Write features have one configured `MutationBoundary`. |
| `SKYFL028` | Success handlers do not repeat an invalidation-only ritual. |
| `SKYFL029` | Refresh rotation is consumed only by session/client doors. |
| `SKYFL030` | Navigation targets do not escape typed routes through dynamic/Object casts. |
| `SKYFL031` | Form submit carries an explicit invalid path. |
| `SKYFL032` | App form fields surface validator/error state. |
| `SKYFL033` | Every `@verify` has one executable co-located `@avp`, and no proof is orphaned. |
| `SKYFL034` | No skipped, disabled, or focused Dart test. |
| `SKYFL035` | Every feature reciprocally links complete semantic happy/sad E2E flows. |

Interactive warnings (`SKYFL008`, `028`, `031`, `032`) become errors at the strict release boundary. All other
findings are errors immediately. The rule-contract suite pins a firing violation for every ID, while clean scaffold
fixtures pin allowed shapes.

## Generate versus scaffold

Only contract wire is regenerated. ViewModels, Views, tests, ARB catalogs, client/session/mutation seams, tokens,
and the UI kit are one-shot scaffolded source owned by the application. Nothing re-emits or overwrites behavior.
There is no Skies widget runtime, MVVM base class, router adapter, service locator, or styling DSL.
