# Skies — Frontend conventions (React web)

The frontend harness is the doctor's soul in a different body: plain, idiomatic **React / TypeScript for the web**,
enforced by an optional ESLint plugin instead of Roslyn. Mobile is Flutter (which can serve the web too, when a
product wants one codebase for every surface): see [FLUTTER-CONVENTIONS.md](FLUTTER-CONVENTIONS.md). The backend
conventions are [CONVENTIONS.md](CONVENTIONS.md).

It exists to kill one failure: **the AI says "done" and ships a screen rendering mocked data.** The harness makes
"wired" the only legal shape and "mock" structurally visible.

## The laws, for the frontend

1. **Stranger-maintainable.** Plain React. MVVM is a *naming discipline over custom hooks*, never a framework: no
   classes, observables, or two-way binding (Angular/WPF idiom).
2. **Doctor-removable.** Remove the ESLint plugin and the wrapped generator and the app still builds and runs; the
   generated client is committed TypeScript.

## The stack

- **React + TypeScript strict**: `tsc` is part of the doctor; it makes "wired" decidable. `skies doctor` runs a
  typecheck leg beside every React package's lint, in parallel: the package's `typecheck` script when it has one,
  else `tsc --noEmit -p <package>` when it has a `tsconfig.json`, and each TypeScript error is a finding (`TS2322`
  at its file and line). A View bound to a renamed hook field lints clean; only the compiler sees it.
- **A router with typed routes**: TanStack Router (route tree) or React Router (route types). Route files (under
  `app/`, TanStack Router's file-based `src/routes/**`, or React Router's `routes/`) are thin shells rendering exactly
  one feature's `*.view.tsx`. Navigation is the router's job, data the ViewModel's: two seams, never crossed.
- **TanStack Query** holds all server state: it is the Model. **orval** (target `react-query`) generates typed
  slice-hooks from the API's OpenAPI (`/openapi/v1.json`). **zod** + **react-hook-form** for forms.
- **Vitest** (jsdom) runs a ViewModel (`renderHook`) and View; Playwright drives the browser when a failure mode
  needs the real page. Cases live in [specs](#specs--every-test-lives-in-one).
- Browser capabilities a ViewModel needs (storage, clipboard, geolocation) enter as injected ports.

**Start a web package with `skies g web-app <Name> [--path clients/web]`.** It writes a plain Vite + React +
TypeScript package on this stack, runnable before any backend exists: TanStack Router with a code-based route tree
(`src/routes/router.ts`, typed `Register`), a QueryClient and the client seams from `lib/` (`skies-client.ts`,
`query.ts`, `feedback.ts`), react-i18next (`src/i18n.ts` over `skies i18n`'s resources), react-hook-form + zod,
`@skiesjs/react`, an app-owned `src/ui/` kit with web DOM props (`onClick`, `onChange` with the change event), the
`@/` alias for `src/`, and `@skiesjs/eslint-plugin`'s recommended config. Its scripts are `dev`, `build` (`vite
build`), `typecheck`, `lint`, and `test` (Vitest over the app's `.specs/*/e2e/`). Inside a Skies app it is declared in
`Skies.toml` (the product's `frontend` and its folder in `[workspace] root`), its `npm ci` joins the CI before
`skies doctor`, and a job builds and tests it. The router is TanStack Router because its code-based tree types routes
with no code generation step; an app that prefers React Router swaps it in the one route file.

## The MVVM convention — one feature, one shape

```
features/<zone>/<name>/
  <Name>.view.tsx        # the View: pure render, consumes exactly one ViewModel
  <Name>.viewModel.ts    # the ViewModel: the only data door, composes generated slice-hooks
  <name>.i18n.ts         # the feature's i18n namespace (one export per app locale)
  panels/ | steps/       # sub-views of multi-panel/multi-step features, same rules
```

- **The suffixes are the analyzer anchor**, as `.ctx.md` is on the backend.
- **`<zone>` is the audience, not the backend domain**: the feature tree mirrors the route tree and how the product
  is experienced (`features/{host,traveler,account,shared}/<name>/`). The generated client already carries the
  domain axis. Rules match by filename, so depth is free; single-persona apps stay flat (`features/<name>/`).
- **The ViewModel is a plain custom hook**, never a class, and render-agnostic (no JSX, no `react-dom`: `SKYFE004`).
- **The View is pure render**: no data access, a function of the ViewModel's return, so mock-free by construction.
- **TanStack Query is the Model.** The ViewModel composes the generated hooks directly; wrapping every query in
  ceremony is the frontend's `IRepository` (`SKY0006`). **One ViewModel per screen, never per query.**
- **Mandatory states.** A ViewModel exposing server data exposes `loading`, `error`, and `empty` explicitly (`SKYFE007`). The
  spine (`@skiesjs/react`) projects each query through `toAsyncState` into the closed `AsyncState<T>` union
  (`combineAsyncStates` folds several: `error > loading > empty > ready`, combined retry), and the View renders it
  through `<Resource>` (`SKYFE010`). Routes project raw params through `requiredParam` (`missing | ready`)
  (`SKYFE018`).

The ViewModel is the slice's twin: `useModel(params) → { state, ...commands }` like `Handle(Input) →
Result<Output>`, Query direct like `DbContext` direct, the View a thin wire like `Map`, the generated hook the
contract like the `Input`/`Output` records.

## The data layer — generated wire, hand-owned behavior

The backend's `Input`/`Output` records become OpenAPI, and the generated client turns each slice into one typed
TanStack hook. An invented endpoint has no exported hook, so **`tsc` does not compile**: completeness is the
compiler, not a rule.

```
client.gen/              # GENERATED, never edited, committed verbatim: one hook per slice (useDeposit, …)
lib/skies-client.ts      # the orval mutator: auth, base URL, `X-Client: web` (cookie session), error mapping
lib/query.ts             # the QueryClient factory with the write-side defaults (SKYFE027)
lib/feedback.ts          # the feedback seam: one door for toasts, wired by the shell at boot
orval.config.ts          # the shipped convention config
```

- **The generator is stock orval, wrapped** (`skies g client`): our own generator would re-solve what orval
  maintains. The opinion lives in the config and the mutator. The generated layer is boring on purpose.
- **`client.gen/` holds generated files only.** `skies g client` checks everything before it writes anything (the
  contract is built, orval is installed in the package or a hoisting ancestor, and every file under `client.gen/`
  carries orval's header), then writes the missing seams and runs orval with `clean: false`. After the run it
  removes only the generated files the contract no longer produces; a hand-written file there stops the run
  instead of being overwritten or deleted.
- **Endpoint name = slice name.** `MapPost("/deposit", …).WithName("Deposit")` → `operationId` → `useDeposit`
  (`SKY0012`).
- **The audience filter is generator config.** orval includes only `app` endpoints; `Asset`, `Webhook`, and
  `Internal` kinds (below) never enter `client.gen/`.

## Pagination — one page shape, two pager hooks

The spine's `Page<T>` (`{ items, totalCount, pageNumber, pageSize }`) is **structural**: it imports nothing from
`client.gen/`, so any generated response carrying the backend's pinned shape matches. On top of it:

- **`toPageInfo(page)`**: pure render facts (`pageCount`, `from`–`to`, `totalCount`, `hasPrev`/`hasNext`);
  `undefined` in, `undefined` out.
- **`usePager()`**: the **numbered pager**. Owns `page` and a debounced, trimmed search term (a settled change
  rewinds to page 1); `next(pageCount?)` clamps once the total is known, `prev` floors at 1.
- **`useAccumulatedPages({ keyOf, resetKey })`**: the **load-more fold**. REPLACE on page 1, APPEND with per-key
  dedupe after (the fresh copy wins); `resetKey` scopes it, `hasMore` compares to `totalCount`, `reset()` rewinds.

**The hooks own state, never the request.** Neither calls the generated client; the ViewModel stays the one data
door (`SKYFE002`) and wires both ends:

```ts
const pager = usePager();
const query = useListWallets(
  { page: pager.page, pageSize: PAGE_SIZE, q: pager.debouncedQ || undefined },
  { query: { placeholderData: keepPreviousData } },
);
const info = toPageInfo(query.data?.wallets); // clamp at the seam: next: () => pager.next(info?.pageCount)

const acc = useAccumulatedPages<ReviewView>({ keyOf: (r) => r.id, resetKey: pointId });
const reviews = useListPointReviews(pointId, { page: acc.page, pageSize: PAGE_SIZE }, /* … */);
const { items, hasMore } = acc.fold(reviews.data?.reviews);
```

- **The two-step `fold` is acyclic**: the hook owns the `page` the query needs; the query owns the response the fold
  needs. Pass `fold` the page **straight off `query.data`** (a page rebuilt inline every render re-folds forever) and
  project items after folding.
- **Guard load-more with `isFetching`**: the hook cannot see in-flight state; a double click skips a page.
- **Which hook**: the pager when the user *navigates* the set (admin tables, search); the fold when the user
  *consumes* it head-first (feeds, reviews). `keepPreviousData` is the default for both; the fold ignores the
  lingering placeholder page across a `resetKey` change.

## Forms — react-hook-form, validation grounded in the contract

Multi-field forms use **react-hook-form**, never a hand-rolled engine (field subscriptions, dirty/touched state, one
submit path). Plain `useState` is fine for a one- or two-field box.

- **`useForm` lives in the ViewModel** (form logic, not rendering), which exposes `control` and a `submit`; panels
  bind with `<Controller>`.
- **Validation is grounded in the contract.** Field shapes and closed enums are enforced by the form type (built from
  generated enums) and controlled pickers. The zod resolver adds only what types cannot: required fields and
  documented `@pattern`s lifted verbatim from the contract. Hand-authored because orval v7's `client: "zod"` emits
  invalid `zod.number().regex(...)` for numeric fields with a `pattern`; revisit when it can.
- **Big forms decompose panel-per-tab**: the ViewModel plus a tab-shell View with a panel registry, and one pure
  `panels/<X>Panel.view.tsx` per tab binding the shared `control`.

### Validation is never silent — `submitOrReveal` (SKYFE031/032)

A validation failure happens *before* the mutation, so `SKYFE013`/`SKYFE027` never see it, and
`handleSubmit(onValid)` runs no code on it: a field failing on a hidden tab makes Save do nothing at all.

- **The submit carries its invalid path** (`SKYFE031`, warn). The spine's `submitOrReveal` requires `onInvalid` and
  resolves the **first invalid field** so the shell can navigate to it:

  ```ts
  const submit = submitOrReveal(form.handleSubmit, (values) => mutation.mutate(toInput(values)), {
    onInvalid: (first) => feedback.error(t("validation.fixHighlighted")),
    order: FIELD_ORDER, // the form's visual order
  });
  const first = await submit();
  if (first) setTab(FIELD_TAB[first]); // single-screen form: reveal = focus
  ```

- **Every `<Controller>` surfaces its `fieldState`** (`SKYFE032`, warn):
  `render={({ field, fieldState }) => <Input … error={fieldState.error?.message} />}`. `error` on an unvalidated
  field is inert, so the rule is near-noise-free.

Both are warnings (a single-screen form with all inline errors visible may use `handleSubmit(onValid)`) and promote
to error together. The canonical instance is the sample's `Deposit.viewModel.ts`.

## Mutations — the write-side defaults (invalidate + feedback)

When ViewModels hand-roll `onSuccess: refetch`, the ones that forget show stale lists with no toast. The fix is not
a second store (it would duplicate server state); it is defaults in the QueryClient's `MutationCache`:

- **Write = the world is stale.** The app's QueryClient (`lib/query.ts`, `SKYFE027`) calls
  `queryClient.invalidateQueries()` on every successful mutation: every query marked stale, active ones refetched.
  Cheap for a business app and always correct: no screen can forget, because none is asked to.
- **Every outcome surfaces.** The same cache posts a success note through the **feedback seam** (`lib/feedback.ts`,
  wired once at boot via `wireFeedback`; nothing below the shell imports a toast lib) and routes every failure
  through it unconditionally. With the defaults wired, set `mutation-error-handled: ["error", { globalSurface: true
  }]`: a bare `.mutate()` is surfaced, and only a swallowing `onError: () => {}` stays flagged.
- **`meta: { silent: true }` skips the success note** (sign-in, drag reorder: the UI change is the feedback). There
  is no silent flag for errors.
- **Targeted invalidation and optimistic updates are opt-in** layers above the default. The hand-rolled
  `onSuccess: () => refetch()` is pure redundancy; `SKYFE028` (warn) reveals it for deletion.
- **`meta: { expectedFailure: true }`** is the one error opt-out, for a failure that *is* a modeled, visible state
  (an anonymous visitor's refresh probe failing is the login screen). Never a way to hide a real failure.

## Session restore — one rotation path (SKYFE029)

The refresh credential is an httpOnly cookie. Parallel rotation burns it: the backend's theft detection sees a spent
token replayed and revokes the whole session family. So restore has **one door**:

- **The seam's `bootstrapSession`, injected into the client.** The scaffolded mutator's interceptor, on a 401
  outside the auth routes, calls the refresher registered with `setTokenRefresher(session.bootstrapSession)` once and
  replays the request: mid-session expiry restores transparently, and an anonymous caller settles to 401 at once
  (pair with a no-retry-on-401 read policy in `defaultOptions`).
- **The same door at boot.** An F5 drops the in-memory bearer while the cookie survives, so the app root runs
  `useSession(session.bootstrapSession)` and gates the navigator on `ready`; boot and interceptor share one
  single-flight.
- **Never two doors** (`SKYFE029`): the refresh hook/operation, or a hand-rolled POST to a refresh route, is
  consumable only inside `lib/skies-client` / `lib/session`.

## Sign-in is an identity change, not a rotation — the seam's two resets

The session seam (`lib/session`, `createSessionSeam`) writes the token through one door (`SKYFE016`) paired with a
cache reset. Two kinds of write need two resets:

- **Rotation**: the *same* identity gets a fresh token (boot `bootstrapSession`, a 401 refresh). Only session-shaped
  queries (`me`) re-read; the screen stays warm. → `onSessionChanged`.
- **Identity change**: a *different* user may hold the session (`signIn`, `clearSession`). The prior user's whole
  cache must be **wiped**, or it bleeds into the next session. → `onIdentityChanged`.

```ts
export const session = createSessionSeam({
  setAccessToken,
  refresh: () => refresh(), // the refresh cookie rides the request
  onSessionChanged: () => queryClient.resetQueries({ queryKey: getMeQueryKey() }), // rotation: light
  onIdentityChanged: () => queryClient.clear(),                                    // identity: total
});

await session.signIn(loginResult); // identity door → onIdentityChanged
await session.bootstrapSession();  // rotation door → onSessionChanged
await session.clearSession();      // identity door → onIdentityChanged
```

`signIn` is the only authentication entry and `onIdentityChanged` is required: falling back to the light reset
recreates the cross-user cache leak. No compatibility alias or weaker fallback exists for this boundary.

## Route guards are symmetric — `guardSession`, one primitive both ways

`SKYFE017` polices a guard's shape but cannot see an **absent** guard, and the one apps forget is the guest guard on
public routes (a signed-in user reaching `/login` or sign-up). `guardSession` is a pure, router-agnostic decision
(it returns data, never navigates), so both guards are the same call with `allow` flipped:

```ts
export type GuardOutcome<Href> =
  | { action: "wait" }                       // session still loading → render a splash
  | { action: "render" }                     // allowed
  | { action: "redirect"; to: Href };        // rejected → redirectTo

guardSession(session, { allow: "authenticated", redirectTo: "/login" }); // private route
guardSession(session, { allow: "anonymous", redirectTo: "/home" });      // guest route
```

Bind it to the router once (a ~10-line component with the splash and `<Navigate>`) and write `<AuthRoute>` /
`<GuestRoute>` from it. No rule for the absent guest guard: `signIn` lives in the login ViewModel and the guard in a
layout route, so a per-file rule would false-positive every correctly guarded app.

## Session resilience — the four ways an auth session breaks

- **A transient `me` failure is not a sign-out.** `toSessionState`'s required `isUnauthorized` classifier
  (`error?.response?.status === 401`) maps 5xx/timeouts to `loading` (retried) and only 401/403 to `anonymous`.
- **A hung bootstrap must not pin the splash.** `useSession(bootstrap, { timeoutMs })` opens the gate after the
  timeout and lets the guard decide; a late success still sets its token first. Omitted ⇒ wait forever.
- **A cold load fires one rotation.** StrictMode double-invokes effects and boot can race the interceptor, so
  `bootstrapSession` is wrapped in `singleFlight(fn)` (concurrent callers share one execution; reopens on settle).
  Never wrap a per-argument operation with it: coalesced callers share the first result.
- **A role gate is the same guard with a predicate**: `allow: (u) => u.role === "admin"`. Authorization is a fact
  about user data, not a second identity axis. Anonymous visitors redirect without calling the predicate.

## Endpoint kinds — the wiring vocabulary

`WithEndpointKind(EndpointKind.X)` on a slice's `Map` (`Skies.Framework.AspNetCore`; a minimal-API lambda cannot
carry a class attribute) classifies an endpoint's **nature**, not a suppression. It tags the endpoint in OpenAPI and
orval's audience filter drops non-app kinds. The default `App` needs no call (a Skies app is UI-first) and must be
wired; `Asset` is a browser-loaded file URL carried by another contract; `Webhook` a third-party callback, never UI;
`Internal` server-to-server. Multiple app audiences are separate manifest surfaces checked as one product union. The
enum carries no configuration (`Webhook(Retries = 3)` is forbidden): retry, signature, and idempotency live visibly
in the `Handle`.

## Server-driven actions — a closed kind, never a client route

When the backend drives navigation, the contract carries **which action** as a closed enum and the client owns
**where it goes**. A server-minted route string is invisible to OpenAPI, `tsc`, and typed routes, and has shipped
404s (backend half: [CONVENTIONS.md](CONVENTIONS.md#the-contract-never-mints-a-client-route)). The client maps over
the **generated** enum:

```ts
import { PendingKind } from "@/client.gen/model";

const PENDING_ROUTE: Record<PendingKind, AppRoute> = { // AppRoute: the router's typed path union
  [PendingKind.CompleteListing]: "/host/properties/new",
  [PendingKind.AcceptTerms]: "/onboarding/host/intermediation-terms",
};
// a new kind breaks this Record until mapped; each value is a typed route
const openPending = (p: Pending) => navigate({ to: PENDING_ROUTE[p.kind] });
```

`SKYFE030` keeps anyone from casting a raw server string into `navigate`. **Config pair**: typed routes on (TanStack's
route tree, React Router's route types); without them the literal degrades to an unchecked `string`.

## The bright line — generate vs scaffold (the law)

| | **Generate** (re-emitted, never edited) | **Scaffold** (`g`, runs once, then yours) |
|---|---|---|
| Contract types (`Input`/`Output` → TS) | ✅ plumbing | |
| Typed slice-hook (`useDeposit()`) | ✅ plumbing | |
| ViewModel body (state, commands, UX) | ❌ source generation of behavior | ✅ a visible skeleton you write |

Scaffolding writes visible code you own and edit, and deleting the generator touches nothing. Source generation
owns its output, clobbers edits, and hides behavior in the generator. `skies g feature <Name> [--kind list|form]`
scaffolds the `view`/`viewModel`/`i18n` unit once, typed from the contract, behavior left to the app; tests are not
scaffolded (cases live in the spec). The unit lands in a kebab-case folder named after the feature
(`src/create-product/`), which is also its i18n namespace, and the scaffold reads the backend's OpenAPI contract (the
one `dotnet build` writes and `skies g client` generates from) so it binds to what orval generates:

- **`--kind list`** (the default: a module's first screen is usually the read of what it holds): the list slice's
  page folded into `AsyncState` and rendered through `<Resource>` with loading, error, and empty states. `Products`
  reads `ListProducts` (`useListProducts`), the plural name `g crud` gives its list; the page field
  (`data.products.items`) and the row type (`ProductView`, from `@/client.gen/model`) come from the contract. Without
  a contract the row is a placeholder to replace.
- **`--kind form`**: a command screen, the Deposit recipe. The ViewModel owns a react-hook-form `useForm` with a zod
  schema restating only the slice's rules, submits through `submitOrReveal` into the `<Name>` mutation, and exposes
  `submitting`, a localized `submitError`, and `completed`; the View renders one `Controller` + `Field` per input,
  the `role=alert` command error, and the success surface. The inputs are the command's body fields from the
  contract, in its order (every input holds a string; numbers convert at the submit). What names the record the
  command acts on, its path and query parameters and a body `version` (the concurrency token `g crud` sends back), is
  the screen's `target`, passed in by the route and sent as given: `useUpdateProductModel({ id, version })` submits
  `mutate({ id, data: { name, price, version } })`. A command with nothing to type (a delete) scaffolds as a
  confirmation with no form. Without a contract, name the inputs with `--fields name:string,price:number` (`string`,
  `number`, `integer`, `uuid`) or the scaffold stops; a field it cannot render as a text box (a boolean, an enum, a
  list) stops it too, by name.

Both import their hook from the package's own client module (`@/client.gen/<backend>`, e.g. `@/client.gen/sample`
for `Sample.Api`), read from `client.gen/` or from `Skies.toml` before the client exists, never from the feature's
name, and import only what a `g web-app` package provides (`@/ui`, `@/i18n`, `@/client.gen`). "Smart stubs" that
pre-fill the body with runtime calls are out. When the contract changes, `*.gen.ts` regenerates and `tsc` breaks the
ViewModel where it is now wrong; you fix it by hand.

## The harness — rule catalog (`SKYFE*`)

`@skiesjs/eslint-plugin`, run by `npm run lint` and `skies doctor`. With MVVM the policed surface is the ViewModel:
the View is mock-free by construction and completeness is the compiler. Every rule comes from drift observed in a
real application. The routing rules (`SKYFE015`–`019`, `022`, `030`) recognize TanStack Router and React Router idioms
and route files in any of their layouts: `app/`, `src/routes/**`, `routes/` (the generated `routeTree.gen.ts` and
tests are not routes). Each rule was calibrated on real apps
([decision](decisions/skies-5-rule-calibration.md)).

**One catalog, two ecosystems.** The rules that exist are the ones `@skiesjs/eslint-plugin` registers (`index.cjs`)
and the Flutter doctor catalogs (`RULES` in `cli/src/flutter/rules/mod.rs`); `cli/src/doctor/catalog_tests.rs` pins
this table and the [Flutter catalog](FLUTTER-CONVENTIONS.md#flutter-doctor-rule-catalog) to them. A number up to 036
means one rule in both: `SKYFE016` and `SKYFL016` share the intent and the tier, and differ only in the ecosystem's
spelling. `SKYFL037`–`040` (the native a11y floor) are Flutter-only; the web's floor is jsx-a11y.

| Rule | Enforces | Why |
|------|----------|-----|
| `SKYFE001` | View purity: a `*.view.tsx` imports no data layer (generated hooks, the client, `fetch`/`axios`); type-only contract imports exempt | keeps the View mock-free |
| `SKYFE002` | Only `*.viewModel.ts` (plus the `lib/session`/`lib/guards` seams) consume generated operations; re-exporting them (`export … from "client.gen"`) elsewhere is flagged; contract types and generated enums stay free, and so do tests and spec cases (they exercise the door) | one data path, one policed surface |
| `SKYFE003` | No import from `**/__mocks__`/`**/fixtures`/MSW outside `*.test.*` | fixtures shipped as data |
| `SKYFE004` | A ViewModel (`*.viewModel.ts`/`.tsx`) is render-agnostic: it renders no JSX and imports no `react-dom` (type-only imports exempt) | ViewModel testable without rendering |
| `SKYFE007` | A ViewModel exposing server data (a generated query hook's `data`) exposes its states: `toAsyncState`/`combineAsyncStates` (`AsyncState`), or a query's pending and error flags projected beside it. Asked once per ViewModel: a secondary read of a query the screen already loads passes; a mutation's `data` (its success surface) is a command, not a resource | failures need an explicit state |
| `SKYFE010` | A `*.view.tsx` routes loading/error/empty through `<Resource>`, not raw `isPending`/`isError` | no async state forgotten |
| `SKYFE011` | Every locale catalog in a `*.i18n.ts` declares the same keys, compared as flattened paths (`empty.title`). A catalog is named like a locale: sibling objects (`ptBR`, `en_US`, `en`) or the children of one locale-keyed object (`{ "pt-BR": {…}, "en-US": {…} }`); lookup maps beside them are not compared | no string ships untranslated |
| `SKYFE013` | A `.mutate()`/`.mutateAsync()` in a ViewModel routes its failure somewhere (`onError`, a read `.isError`, try/catch or `.catch()`, a propagated return); an empty `onError: () => {}` is flagged. With `{ globalSurface: true }` (the SKYFE027 defaults wired), a bare `.mutate()` passes and only the empty handler is flagged | no silent failure, no `onError` theater |
| `SKYFE014` | User-facing JSX text and copy props (`placeholder`, `label`, `title`, `aria-label`, `alt`…) in a View go through `t()` | feeds the catalog SKYFE011 keeps complete |
| `SKYFE015` | No `router.navigate`/`useNavigate()` call inside `useEffect`; redirect-on-state is declarative (`return <Navigate … />`). Navigation on a user action stays allowed. Views and routes only | effect redirects flash and loop |
| `SKYFE016` | Outside `lib/session`/`lib/guards` (and tests), no import of a token setter (`setAccessToken`, `setToken`, `setSession`) and no `localStorage`/`sessionStorage` write of a session key: a key one of whose words is `token`, `jwt`, `session`, or `auth` (`accessToken`, `auth_token`; not `authorName`). The write goes through `lib/session`, paired with the `me` reset | a scattered write forgets the reset |
| `SKYFE017` | In a route or guard file, a redirect (`return <Navigate/>`, `return`/`throw redirect(…)`) is never decided on a raw auth boolean (`isAuthenticated`, `isLoggedIn`, `isSignedIn`…); branch on a `SessionState` (`loading \| authenticated \| anonymous`) | a boolean reads "loading" as "signed out" |
| `SKYFE018` | A route reading a required id through a loose `useParams()` (React Router bare, TanStack `{ strict: false }`) guards its absence with a declarative redirect: `requiredParam()` (`if (id.status === "missing") return <Navigate/>`) or `!id`; a `throw` on that test (`throw notFound()`) or `invariant(id)` also guards, `return null` does not. Strict TanStack reads are exempt | a param-less hit renders a ghost screen |
| `SKYFE019` | No bare `history.back()` (`window.history` or TanStack's `router.history`) or `navigate(-1)`; use `safeBack` / an app `useGoBack` that falls back to a parent | a dead Back button on deep links |
| `SKYFE020` | The API base URL comes from configuration (`VITE_API_URL`, a relative base, an injected default), never a host in `axios.create({ baseURL: "http://…" })`; the backend pins its dev port in `launchSettings`. A tool's own config (`playwright.config.ts`'s `use.baseURL`) is not the client | front and API ports drift apart |
| `SKYFE021` | No `dangerouslySetInnerHTML` outside the audited `lib/html` seam, where any sanitizer lives | raw HTML is the XSS door |
| `SKYFE022` | Never navigate to a URL-derived value (`navigate({ to: returnTo })`, `<Navigate to={next}>`, `location.href = next` off `useSearch`/`Route.useSearch()`/`useSearchParams`) without mapping it through an allowlist of in-app routes. Only the target counts: a value forwarded in `search`/`params`, a lookup key (`ROUTES[next]`), or a conditional's test is not flagged | open redirect, the phishing primitive |
| `SKYFE023` | Warning. No unfinished placeholder in production code (tests, specs, and `client.gen/` exempt): an uppercase `TODO`/`FIXME`/`HACK`/`XXX` or "wire later" comment, a `@ts-expect-error`/`@ts-ignore`, or a `throw new Error("not implemented")` stub | "almost done" is not done |
| `SKYFE027` | Writes run through the app's one mutation surface with the write-side defaults wired: every production `new QueryClient(...)` carries `mutationCache: new MutationCache({ onSuccess, onError })`: success invalidates and posts the note (`meta.silent` opts out), failure goes through the feedback seam. Tests and `test/`/`test-utils/` build bare clients freely. Scaffolded as `lib/query.ts` | stale lists after a write, no toast |
| `SKYFE028` | Warning. In a ViewModel, an `onSuccess` whose whole body is refetch/invalidate calls (inline, named, or `useCallback`-wrapped) duplicates SKYFE027; delete it. Handlers doing more (navigate, reset, hand off an id) are never flagged | a cargo-culted ritual |
| `SKYFE029` | The refresh hook/operation (or a hand-rolled POST to a refresh route) is consumed only in `lib/skies-client` / `lib/session`; type-only imports free | two rotations burn the session family |
| `SKYFE030` | No `as never`/`as any`/`as unknown` on a `router.navigate`/`useNavigate()` argument or a `<Navigate>`/`<Link>` `to`. Pass a typed literal or `{ to, params }`. Config pair: typed routes on | a cast lets a drifted route 404 in production |
| `SKYFE031` | Warning. In a `*.viewModel.ts`, a one-argument `handleSubmit(onValid)` is flagged; use `submitOrReveal(form.handleSubmit, onValid, { onInvalid })` or pass `onInvalid`. Promotes with SKYFE032 | Save goes mute on a hidden invalid field |
| `SKYFE032` | Warning. A validated field shows its error where the control is: a `<Controller>` whose inline `render` never reads `fieldState` is flagged (a resolver's error reaches the field only through it); pass `error={fieldState.error?.message}`. A non-inline surface must still expose the error explicitly | a field's error has no surface |
| `SKYFE036` | A `test`/`it`/`describe` call (or a declaring member: `test.describe`, `it.each([…])(…)`, `describe.skip`) from `vitest`, `@playwright/test`, `@jest/globals`, `bun:test`, `node:test`, a global, or the app's re-export of an extended runner (called with a title and a body), in a file with no `.specs/` path segment is flagged, once per file. A local function of the same name is not a test, nor is a fixture file (`test.extend`, `test.step`, hooks). It asks where a test lives, never that one exists | tests as coverage prove nothing |

**Tiers.** An architecture or security rule is an **error**; a taste or review signal is a **warning**, the same
split as the [.NET catalog](CONVENTIONS.md#the-doctor--rule-catalog) and the same tier as each rule's Flutter twin.
`recommended` sets every rule to error except `SKYFE023` (an unfinished-work marker is for review), `SKYFE028` (a
redundant refetch is harmless), and `SKYFE031`/`SKYFE032` (a single-screen form with every error visible inline may
skip `submitOrReveal`). An app promotes those in its own config; it never needs to promote the rest. To suppress one
finding, use `// eslint-disable-next-line skies/<rule> -- <reason>`: one rule, one line, the reason written
([Suppression](CONVENTIONS.md#suppression) covers all three ecosystems).

Numbering gaps are removed rules (the latest, `SKYFE009`, kept ViewModels free of React Native imports). Beside
these, `recommended` carries the [accessibility floor](#accessibility--the-a11y-floor-on-by-default) (third-party
`jsx-a11y/*` ids).

**Severity follows direction.** Front→back (calling an endpoint that does not exist) fails `tsc` for free.
Back→front (an endpoint nothing wires yet) is a legitimate intermediate state, not a rule. **Contract freshness**:
regenerate with `skies g client` when the contract moves; a stale committed client shows as a diff and type errors.

## Specs — every test lives in one

Frontend features are accepted like backend ones: a `.specs/` folder with failure modes, cases in `e2e/`, and a
receipt from `skies proof record` (see [CONVENTIONS.md](CONVENTIONS.md#specs-and-proofs)). The engine is the app's:
Playwright for the real browser, Vitest for a View + ViewModel in jsdom. Name each case after its failure mode
(`test("FM-3: an expired session lands on sign-in")`); nothing in the ViewModel, View, or a manifest points at the spec.

**A spec is the only home for a test** (`SKYFE036`): no `Foo.test.tsx` beside `Foo.viewModel.ts`. An isolated system
(a formatter, a reducer, a UI kit) gets its own spec. Cases import code by relative path or alias.

**Test support belongs to the specs too.** A web spec runs the real client against a stand-in backend at the HTTP
boundary (MSW), whose handlers answer the backend's real routes (`POST /wallets/deposit`, never a guessed
`/deposit`) with its real error codes. The server and its handlers live in one setup file owned by the app's specs,
wired through the Vitest config's `setupFiles`; the sample's is `examples/sample-app/.specs/web.setup.ts`, loaded by
the config its web runner uses. Never in `src/` (MSW there is production code, `SKYFE003`), and never in a framework
file: a feature that needs a new route adds its handler to the app's setup. An app's web runner points at its package
(`skies g web-app` prints this one):

```toml
[runners.web]
setup = "test -d clients/web/node_modules || npm --prefix clients/web ci"
command = "node clients/web/node_modules/vitest/vitest.mjs run --config clients/web/vitest.config.ts --reporter=junit --outputFile={report} {dir}"
```

`setup` matters: red runs in a fresh git worktree with no `node_modules`. A `g web-app` package's `vitest.config.ts`
already runs from the application root over `.specs/*/e2e/**/*.test.{ts,tsx}` (jsdom, the `@/` alias) and resolves a
case's bare imports from the package, so `npm test` there runs every web spec; wire an MSW setup file through its
`setupFiles` when the specs need a stand-in backend. The sample is the one exception to this shape: it lives inside
this repository and borrows `frontend-sdk`'s installed dependencies and Vitest config, so its runner reads
`../../frontend-sdk/node_modules/vitest/vitest.mjs … --config ../../frontend-sdk/vitest.config.ts`, a path that only
exists here.

Tests write artifacts to `process.env.SKIES_EVIDENCE` when set; they become the spec's committed `evidence/`. For an
Assay-decided mode, tag its line (`- FM-4 … [avp: <criterion-id>]`) and write the verdict with
`verdictToJsonLine(verdict)` to `$SKIES_EVIDENCE/avp-FM-4.json`; the mode then passes only with a passing verdict.

## Code comments — the code speaks for itself

Comments are **English** and default to **none**: good names and types say what the code does. A comment says what
the code can't: a non-obvious why, a gotcha, an invariant, a contract quirk. Strip on sight: migration play-by-play
and thinking out loud (`// Step 1: …`, `// re-skinned onto…`; git is the narrative), restating the line below, and
mixed languages (only user-facing copy is localized, and it lives in i18n). Unsure? Delete it.

## i18n — react-i18next, per-feature namespaces

User-facing copy is **never inlined** in a View. One i18next instance (`src/i18n`); each feature owns a namespace
named after its folder (`src/features/<feat>/<feat>.i18n.ts`, one export per locale), assembled by `skies i18n`
into `src/i18n/resources.generated.ts`; shared copy lives in `common`. A View reads `useTranslation("<feat>")`.
Adding a locale changes no namespace. The locale set is the app's: `skies g feature` copies it from the package's
existing catalogs (a single `en` when there are none), and `skies i18n` keys each export by its language (`enUS`,
`en_US`, and `en` all become `en`), refusing a catalog whose locales differ from the others.

**Error codes are translated in every language.** The backend ships each error as a stable `ErrorBody.code`
(`SKY0018`/`SKY0019`); the front owns the copy in an `api-errors` catalog typed against the generated code union, so
a missing code is a type error, and `SKYFE011` keeps every locale in step.

## Accessibility — the a11y floor, on by default

Accessibility is a floor like the .NET CA* security floor: on by default, relaxed explicitly, one rule at a time. It
is not architecture, so it is not a SKYFE rule: [`eslint-plugin-jsx-a11y`](https://www.npmjs.com/package/eslint-plugin-jsx-a11y)
polices `alt`, `aria-*`, `role`, and `href`.

- **`skies.configs.recommended`** (and `flat/recommended`) carries jsx-a11y's recommended set at **error**.
  `eslint-plugin-jsx-a11y` is a dependency of `@skiesjs/eslint-plugin`, so the app installs nothing. Do not register
  `jsx-a11y` again in the same config: ESLint rejects a plugin name bound to two objects.
- **`aria-role` checks the DOM only** (`ignoreNonDOM: true`): a design-system `<Text role="…">` prop is not ARIA.
- **`label-has-associated-control` looks three levels deep** (`depth: 3`) for a label's text, so a card-style option
  (`<label><input/><span><strong>{name}`) is not reported as text-less.
- **Relax one rule** in a later config object, scoped as narrowly as the reason, for a real case, never a backlog:

  ```js
  export default [
    skies.configs.recommended,
    { files: ["src/features/map/**/*.tsx"], rules: { "jsx-a11y/no-static-element-interactions": "off" } },
  ];
  ```

- **Static only**: it reads JSX and never demands a test, an audit, or a manifest.

Flutter's floor is `SKYFL037`–`040`: see [FLUTTER-CONVENTIONS.md](FLUTTER-CONVENTIONS.md#accessibility--the-a11y-floor).

## Scope and non-goals

**In:** the MVVM feature convention, the `SKYFE*` rules, `skies g web-app`, `skies g feature`, and `skies g client`
(stock orval with the shipped config and mutator). One blessed frontend shape for the web.

**Out, by decision:** a bespoke generator; source generation of behavior (ViewModels are scaffolded once and
owned); an MVVM framework; a design system (styling, kit, tokens, and layout are the app's: a framework vocabulary
only adds rules to fight); React Native (mobile is Flutter); TS decorators like `@Slice` (React has no idiomatic
decorator seam; the folder/file convention is the annotation); multi-app sprawl; frontend code in core (the harness
is a separate optional package, never in `Skies.Framework.Abstractions` or `Skies.Framework.Doctor`).

A proposal that adds capability instead of convention and enforcement is a scope violation. Reject it in line.
