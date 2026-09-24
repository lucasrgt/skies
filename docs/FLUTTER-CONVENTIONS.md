# Skies — Flutter conventions

Flutter is a native body for the same product guarantees defined in
[FRONTEND-CONVENTIONS.md](FRONTEND-CONVENTIONS.md). Parity means equivalent capability and enforcement, not a
literal translation of React APIs. Output remains plain, idiomatic Dart and Flutter; deleting the `skies` CLI
leaves an ordinary application that still builds and runs.

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
- Flutter's official [`integration_test`](https://docs.flutter.dev/testing/integration-tests) for spec E2E;
  `flutter_test` for isolated units whose failure modes were written first.

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

## Specs and E2E

Flutter features are accepted like every other Skies feature: a spec folder under `.specs/` with its failure modes,
black-box E2E in `e2e/`, and a receipt from `skies proof record` (see
[CONVENTIONS.md](CONVENTIONS.md#specs-and-proofs)). The E2E engine is Flutter's own `integration_test` (or
Maestro). Declare a runner in `Skies.toml` that runs one spec folder and writes a JUnit report, and name each case
after the failure mode it covers (`testWidgets('FM-2: an expired session lands on sign-in', …)`).

Styling, the widget kit, tokens, and layout are the application's. Accessibility is too; Flutter's
`meetsGuideline` matchers are a good failure-mode check for a spec, not a framework rule.

## Flutter doctor rule catalog

Every number preserves the corresponding `SKYFE` semantic slot; only the ecosystem spelling changes. The doctor
enforces architecture only.

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

`skies doctor` runs these rules natively over every Flutter package declared in `Skies.toml`. `SKYFL028`, `031`,
and `032` are warnings; every other finding is an error. The numbers keep the corresponding `SKYFE` slots, so gaps
are the React rules that were removed in Skies 5.

## Generate versus scaffold

Only contract wire is regenerated (`skies g client`). ViewModels, Views, ARB catalogs, and the client/session/mutation
seams are one-shot scaffolded source owned by the application. Nothing re-emits or overwrites behavior.
There is no Skies widget runtime, MVVM base class, router adapter, service locator, or styling DSL.
