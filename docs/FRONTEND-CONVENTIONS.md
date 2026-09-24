# Skies (.NET) — Frontend Conventions & Harness

The frontend harness is the **same soul as the doctor, a different body**. Same mentality —
convention over configuration, semantic density, enforcement that an LLM cannot drift past —
but the body is plain, idiomatic **React / TypeScript for the web**, not C#, and the harness is a
separate, optional, TS-world tool, not the Roslyn doctor.

Mobile is Flutter. A Skies product ships its phone apps (and, when it wants one codebase for every
surface, its web app too) in Flutter, with the same guarantees spelled in Dart: see
[FLUTTER-CONVENTIONS.md](FLUTTER-CONVENTIONS.md). This file is the React web body.

It exists to kill one class of failure the backend already designed out: **the AI says "it's
done" and ships a screen rendering mocked data.** That failure is documented in our own history
(hostpoint's `WAR-*` workarounds — screens inlining storybook fixtures instead of wired data).
The harness makes "wired" the only legal shape and "mock" structurally visible.

Ground every frontend convention here, never memory. The backend constitution is
[CONVENTIONS.md](CONVENTIONS.md); the decision that birthed this file is
[`skies-framework-frontend-harness`](decisions/skies-framework-frontend-harness.md).

---

## The two laws — restated for the frontend

1. **Stranger-maintainable.** The output is always plain, idiomatic React that a React dev
   who has never heard of Skies can read and maintain. This is why MVVM lives here as a
   *naming discipline over custom hooks*, never as a framework (no classes, no observables, no
   two-way binding — that is Angular/WPF idiom imported into React, and it fails this law).
2. **Doctor-removable.** Remove the harness (the ESLint plugin + the wrapped generator) and the
   app still **builds and runs** — you only lose enforcement. The generated client is committed
   TypeScript that stays; the convention is plain React that stays. The harness is wire, not
   apparatus.

Any feature that fails both — a bespoke compiler, source-gen of behavior, a ViewModel framework
you inherit from — is **out, by construction**. Both are the lessons of the predecessor language (died owning a
compiler) and aerocoding (died generating artifacts for features that did not exist).

---

## The stack — opinionated, web

The choices are pre-made so the AI decides less:

- **React + TypeScript strict** — `tsc` is part of the doctor; it is what makes "wired" decidable.
- **A router with typed routes** — TanStack Router (its generated route tree) or React Router (its generated
  route types). Route files live under an `app/` tree; screens are routes, a route renders exactly one View. The
  routing rules recognize both routers' idioms and depend on neither.
- **TanStack Query** — all server state (cache, mutation, status). It is the Model.
- **orval** (target `react-query`) — generates the typed slice-hooks from the .NET API's OpenAPI
  (`/openapi/v1.json`).
- **zod** + **react-hook-form** — input schemas + form state.
- **Vitest** (jsdom) — the runner for a screen's ViewModel and View: a ViewModel is a hook, exercised with
  `@testing-library/react`'s `renderHook`; a View renders against the app's own component kit in jsdom. Its cases
  live in specs, never beside the code (see [Specs](#specs--every-test-lives-in-one)). Playwright drives the
  browser when a failure mode needs the real page.

The route tree and the feature triple compose: a route file under `app/` is a thin shell that renders one
feature's `*.view.tsx`; the feature folder holds the view and its model (its cases live in a spec). Navigation is
the router's job; data is the ViewModel's. Two seams, never crossed. Browser capabilities a ViewModel needs
(storage, clipboard, geolocation) enter as injected ports, so the ViewModel stays a plain hook its case can drive.

---

## The MVVM convention — one feature, one shape

One screen = a co-located pair (its cases live in a spec). The suffixes are the analyzer anchor, the way
`.ctx.md` is on the backend:

```
features/<zone>/<name>/
  <Name>.view.tsx        # the View — pure render; consumes exactly one ViewModel
  <Name>.viewModel.ts    # the ViewModel — the only data door; composes generated slice-hooks
  <name>.i18n.ts         # the feature's i18n namespace (ptBR/esES/enUS)
  panels/ | steps/       # for multi-panel/multi-step features (sub-views, same harness rules)
```

- **Group features by `<zone>` = audience, not by backend domain.** When an app has distinct personas,
  the feature tree mirrors the **route tree** and how the product is *experienced* — e.g. Hostpoint:
  `features/{host,traveler,account,shared}/<name>/`. The domain axis (Catalog/Operations/…) is already
  carried by the generated client; re-mirroring the backend's modules on the front would scatter a single
  audience across domains. The harness is depth-agnostic (`SKYFE*` match by filename, not folder), so the
  grouping is a free organizational choice. Single-persona apps can stay flat (`features/<name>/`).

- **The ViewModel is a plain custom hook**, never a class: `useDepositModel(params) → { state,
  ...commands }`. Custom hooks *are* React's idiomatic way to extract logic — you get the MVVM
  seam without betraying the grain.
- **The ViewModel is render-agnostic** — no JSX, no `react-dom`. It is testable without
  rendering, exactly as a backend `Handle` is HTTP-agnostic and testable without a host.
- **The View is pure render.** It owns no data access at all; it is a function of the
  ViewModel's return. This makes the View **mock-free by construction** — the harness never has
  to police it for data discipline.
- **TanStack Query is the Model.** The ViewModel *composes* Query (the generated slice-hooks); it
  never hides it behind a wrapper. A ViewModel that re-embeds every query in ceremony is the
  frontend's `IRepository`/unit-of-work — the clean-arch bloat the backend cuts (`SKY0006`).
  **One ViewModel per screen, never per query.**
- **Mandatory states.** A ViewModel exposing server data exposes `loading`, `error`, and `empty`
  as explicit state — never improvised in the View. This is the sad-path discipline of the back.
  The spine (`@skiesjs/react`) carries the primitives: the ViewModel projects each query through
  `toAsyncState` into the closed `AsyncState<T>` union (a multi-query screen folds them with
  `combineAsyncStates` — precedence `error > loading > empty > ready`, combined retry), and the
  View renders it through `<Resource>` (`SKYFE010`). Routes project raw params through
  `requiredParam` (`missing | ready`) before rendering (`SKYFE018`).

The parallel to the slice is near-exact — the payoff is semantic density: the AI reads one
ViewModel and knows the feature, as it reads one slice:

| Backend (slice) | Frontend (feature) |
|---|---|
| `Handle(Input) → Task<Result<Output>>` | `useModel(params) → { state, ...commands }` |
| HTTP-agnostic → testable without a host | render-agnostic → testable without JSX |
| `DbContext` direct, no repository (`SKY0006`) | TanStack Query direct, no wrapper |
| `Map` is the thin wire of transport | the `View` is the thin wire of render |
| `Input`/`Output` records *are* the contract | the generated slice-hook *is* the contract |

---

## The data layer — generated wire, hand-owned behavior

The "wired" guarantee is the type system, not a heuristic. The backend's `Input`/`Output`
records are the contract; ASP.NET emits them as OpenAPI; a generated typed client turns each
slice into one typed TanStack hook. If the AI invents an endpoint that does not exist, the hook
is not exported and **`tsc` does not compile** — the completeness gate is the compiler, not a
rule (the frontend analog of the old `API-HANDLER-UNWIRED-001`: no silent 404).

```
client.gen/              # GENERATED — never edited by hand, committed verbatim
  <slice>.gen.ts         #   one typed TanStack hook per slice (useDeposit, …)
lib/skies-client.ts      # the orval mutator — injects auth, unwraps the body, maps error→state
lib/query.ts             # the QueryClient factory — the write-side mutation defaults (SKYFE027, below)
lib/feedback.ts          # the feedback seam — one door for toasts; the shell wires the sink at boot
orval.config.ts          # the shipped convention config
```

- **The generator is stock, wrapped — never bespoke.** `skies g client` runs **orval**
  (target `react-query`) under our config. We do not own a compiler; we wire an existing one.
  Building our own OpenAPI→TS→TanStack generator *is* the bespoke-compiler gesture — re-solving parsing
  and emission that orval already maintains, tests, and edge-cases for free.
- **The opinion lives in config + convention, not in a fork.** It is two things: the `orval.config.ts`
  we ship and the **mutator** (`lib/skies-client.ts` — the typed Skies client: auth, the injectable base
  URL, the `X-Client: web` header that turns on the cookie session, error mapping). This mirrors the back
  exactly — we do not fork EF Core; we use it stock and direct, and the opinion is the slice convention +
  the doctor.
- **The generated layer is boring on purpose.** All semantic density lives *above* it, in the
  hand-written ViewModel. Never poison the generated hooks.
- **One backend micro-convention makes the 1:1 clean.** The slice's `Map` names its endpoint —
  `MapPost("/deposit", …).WithName("Deposit")` → the OpenAPI `operationId` → orval emits
  `useDeposit`. The contract of the `Handle` becomes the name of the wire, with nothing bespoke
  in between. (`SKY0012` enforces it: endpoint name = slice name.)
- **Audience filters the client at the generator, not the rule.** orval is configured to include only
  endpoints tagged for *this* frontend's audience (`app`). Webhooks, internal/server-to-server, and
  browser-asset endpoints carry a different `WithEndpointKind(EndpointKind.X)` kind (below), are tagged
  accordingly, and never enter `client.gen/`. The noise is removed in the plumbing (config of a stock tool).
  This is the old skies's "audience SDK projection", done by tool config instead of a bespoke compiler.

---

## Pagination — one page shape, two pager hooks

The backend's canonical `Page<T>` (see `CONVENTIONS.md` — `AddSkiesOpenApi` pins the four members
required and plainly numeric) is what makes a page *recognizable* on this side of the wire. The spine's
`Page<T>` — `{ items, totalCount, pageNumber, pageSize }` — is **structural on purpose**, like
`QueryLike`: it imports nothing from `client.gen/`, so any generated response carrying the shape matches,
whatever the slice, whatever the app. On top of it the spine ships one pure deriver and two stateful
hooks:

- **`toPageInfo(page)`** — the render facts (`pageCount`, `from`–`to`, `totalCount`, `hasPrev`/`hasNext`),
  pure, so the View renders "21–40 de 87" without arithmetic. `undefined` propagates: no page yet → no
  info → no clamp.
- **`usePager()`** — the **numbered pager** (the admin-table case — hostpoint's `PointsList`): owns `page`
  plus a debounced, trimmed search term; a *settled* term change rewinds to page 1 atomically (retyping
  the same term does not); `next(pageCount?)` clamps once the total is known, `prev` floors at 1.
- **`useAccumulatedPages({ keyOf, resetKey })`** — the **load-more fold** (the feed case — hostpoint's
  `PublicPointReviews`): folds arriving pages into one growing list — REPLACE on page 1, APPEND with
  per-key dedupe on the rest (the absorber for items that slide across a page boundary between requests;
  the fresh copy wins in place). `resetKey` scopes the accumulation (a different parent id starts from
  scratch); `hasMore` compares the accumulation against `totalCount`; `reset()` rewinds to page 1 and
  lets the refetched head replace the list, blink-free.

**The golden rule: the hooks are fetch-agnostic — they own STATE, never the request.** Neither hook
wraps, imports, or calls the generated client; the ViewModel remains the one data door (`SKYFE002`) and
wires the two ends itself:

```ts
// numbered (admin list)
const pager = usePager();
const query = useListWallets(
  { page: pager.page, pageSize: PAGE_SIZE, q: pager.debouncedQ || undefined },
  { query: { placeholderData: keepPreviousData } }, // rows stay put while the next page loads
);
const info = toPageInfo(query.data?.wallets);
// …return pager + info + toAsyncState(query…); clamp at the seam: next: () => pager.next(info?.pageCount)

// accumulated (load-more feed)
const acc = useAccumulatedPages<ReviewView>({ keyOf: (r) => r.id, resetKey: pointId });
const query = useListPointReviews(pointId, { page: acc.page, pageSize: PAGE_SIZE }, /* … */);
const { items, hasMore } = acc.fold(query.data?.reviews);
```

- **The two-step `fold` is the acyclic wiring**: the hook (above the query) owns the `page` the query
  needs; the query owns the response the fold needs. Hand `fold` the page **straight off `query.data`**
  (a stable cache identity — a page object rebuilt inline every render re-folds forever) and project
  items for display *after* folding, never before.
- **Guard the load-more button with `isFetching`** (the pilot's `loadingMore`): the fetch-agnostic hook
  cannot see in-flight state, and a double-click would otherwise skip a page.
- **Which hook**: the numbered pager when the user *navigates* the set (admin tables, search — `q`
  belongs here); the accumulated fold when the user *consumes* the set head-first (feeds, reviews).
- `keepPreviousData` is the default posture for both — no blink to a spinner between pages — and it is
  exactly why `useAccumulatedPages` keeps the identity of the last folded page across a `resetKey`
  change: the lingering placeholder page must not fold into the new accumulation.

---

## Forms — react-hook-form, validation grounded in the contract

Multi-field forms (the property/service editors) use **react-hook-form**, not a hand-rolled
`useState` draft. The same "wire, not reinvention" law that keeps us off a bespoke OpenAPI
compiler keeps us off a hand-rolled form engine: RHF gives uncontrolled inputs + field-level
subscriptions (no whole-form re-render per keystroke), dirty/touched/validation state, and one
submit path. Reimplementing that is the gesture the framework forbids.

- **The `useForm` lives in the ViewModel.** It is form *logic*, not rendering. The ViewModel exposes
  `control` + a `submit`; panels bind their slice with `<Controller>`, and the form's rules are tested
  through the ViewModel.
- **Validation is grounded in the contract.** The field *shape* and closed enums are already
  enforced at compile time by the form type (built from the generated enums) and at runtime by the
  controlled pickers — an invalid enum value cannot be produced. The zod resolver adds only what
  the type system can't: required fields and documented `@pattern`s (e.g. the coordinate regex,
  lifted verbatim from the contract). Small, hand-authored, contract-grounded — not free-invented.
- **Why not generate the zod from the contract.** orval's `client: "zod"` output is the ideal
  (schema straight from OpenAPI), but v7 emits invalid `zod.number().regex(...)` for numeric fields
  carrying a `pattern` — it does not compile. Until coordinates are typed as `string` server-side
  (or orval fixes it), forms hand-author the compact schema. Revisit when the generator can.
- **Plain `useState` is fine for trivial input.** A one- or two-field reply/search box does not
  need RHF; the convention is for genuine multi-field forms.

Big forms decompose **panel-per-tab**: a hand-written spine (the ViewModel + the tab-shell View
with a panel registry) plus one pure `panels/<X>Panel.view.tsx` per tab, each a function of the
shared `control`. The panels are independent, so they migrate as panel-granularity fan-out.

### Validation is never silent — `submitOrReveal` (SKYFE031/032)

`SKYFE013`/`SKYFE027` guarantee a **failed mutation** surfaces — but a validation failure happens
*before* the mutation, and RHF's `handleSubmit(onValid)` without the second argument runs **no code
at all** on it. On a multi-tab editor that is the mute Save button shipped to prod: a field failing
on a hidden tab (cep/lat/long on the Address tab) left the button doing literally nothing — no
mutation, no toast, no visible error. Two rules + one primitive close the cycle "a validation error
always shows":

- **The submit always carries its invalid path** (`SKYFE031`, warn). The blessed shape is the spine's
  `submitOrReveal`, which makes the surface impossible to omit (the `onInvalid` option is required)
  and resolves the **first invalid field** so the shell can navigate to it:

  ```ts
  // ViewModel — the surface is forced at construction; `order` = the form's visual order.
  const submit = submitOrReveal(form.handleSubmit, (values) => mutation.mutate(toInput(values)), {
    onInvalid: (first) => feedback.error(t("validation.fixHighlighted")),
    order: FIELD_ORDER,
  });
  // Single-screen form: reveal = focus. Multi-tab shell: the resolved field picks the tab.
  const first = await submit();
  if (first) setTab(FIELD_TAB[first]);
  ```

- **Every `<Controller>` surfaces its `fieldState`** (`SKYFE032`, warn). A render prop that only
  destructures `{ field }` leaves that field's error with no surface even when the form-level toast
  fires — pass it through: `render={({ field, fieldState }) => <Input … error={fieldState.error?.message} />}`.
  Passing `error` on a field without validation is inert, so the rule is near-noise-free.

Both enter **warn** (a single-screen form whose inline errors are all visible is a legitimate
`handleSubmit(onValid)` consumer) and are promoted to error together once the primitive absorbs the
common case. The canonical instance is the sample's `Deposit.viewModel.ts`.

---

## Mutations — the write-side defaults (invalidate + feedback)

The read side has long been covered (`AsyncState` → `<Resource>`, `SKYFE010`); the write side was
convention-by-hope, and a pilot paid for it: **a created category only appeared after F5, with no
toast** — the mutation succeeded on the server while the query cache kept serving the stale list.
30 of 43 ViewModels hand-rolled `onSuccess: refetch`; the 13 that forgot were the bug. When 70% of
the code repeats a ritual and 30% forgets it, that is the operational definition of a missing
default. The answer is **not** a second store (zustand/redux would duplicate server state into a
second cache and double the desync surface — TanStack Query *is* the Model); it is pinning the
write-side defaults where TanStack designed them to live:

- **Write = the world is stale.** The app's QueryClient (scaffolded as `lib/query.ts`, enforced by
  `SKYFE027`) carries a global `MutationCache`: on every successful mutation it calls
  `queryClient.invalidateQueries()` — every query marked stale, the **active** ones refetched
  immediately. For a business app this is cheap and **always correct**: no screen can forget to
  invalidate, because no screen is asked to. The safe, slightly-wasteful default; Rails would smile.
- **Every outcome surfaces.** The same cache posts a success note through the **feedback seam**
  (`lib/feedback.ts` — the one-door shape of `SKYFE016` applied to toasts; the shell wires the
  app's toast lib once at boot via `wireFeedback`, nothing below the shell imports a toast lib) and
  routes every failure through it **unconditionally** — the global half of `SKYFE013`. With the
  defaults wired, the app sets `mutation-error-handled: ["error", { globalSurface: true }]`: a bare
  `.mutate()` is surfaced by construction, and only the actively-swallowing `onError: () => {}`
  stays flagged.
- **`meta: { silent: true }` opts out of the success note** (a sign-in, a drag reorder — the UI
  change *is* the feedback). There is deliberately no silent flag for errors: a mutation failure
  always surfaces, and a screen that also reads `.isError` just adds a richer inline surface on top.
- **Targeted invalidation / optimistic updates are the opt-in, not the baseline.** A screen that
  *proves* it needs surgical `setQueryData`/optimistic UX layers it above the default. What dies is
  the hand-rolled `onSuccess: () => refetch()` ritual — with the defaults wired it is pure
  redundancy, and `SKYFE028` (warn) reveals it so it gets deleted instead of cargo-culted into the
  next screen.

The parallel to the backend is exact: a slice's `Handle` does not opt into transactionality or
error mapping per call site — the boundary owns it once. The frontend's write boundary is the
`MutationCache`; this convention just moves the two defaults (cache coherence, outcome feedback)
to where the boundary already is.

**The one error opt-out: `meta: { expectedFailure: true }`.** A mutation failure always surfaces —
except when the failure *is* a modeled, visible state the screen renders. The canonical case: an
anonymous visitor's refresh probe failing IS the login screen, not an error to toast. The flag's
name carries the bar: it marks an *expected* outcome the UI already shows, never a way to hide a
real failure (an empty `onError` stays flagged by `SKYFE013` regardless).

---

## Session restore — one rotation path (SKYFE029)

The refresh credential — an httpOnly cookie the browser sends with the refresh request, invisible to page script
by design — is **burned by parallel rotation**: the backend's theft detection sees a spent token replayed and
revokes the whole session family. So session restore is a **one-door** discipline, the SKYFE002/016 shape
applied to rotation:

- **The one door: the session seam's `bootstrapSession`, injected into the client interceptor.** The
  scaffolded mutator (`lib/skies-client.ts`) ships `setTokenRefresher(fn)` and an interceptor that, on a
  401 outside the auth routes, calls the injected refresher once and replays the request. The shell registers
  the seam's `bootstrapSession` as that refresher at boot (`setTokenRefresher(session.bootstrapSession)`), so
  the rotation logic — **single-flight**, an empty post the cookie rides — lives in exactly one place, the seam,
  never forked into the transport file. A mid-session expiry restores transparently inside the first attempt: no
  anonymous flash, no bounce to login, and a genuinely anonymous caller settles to 401 at once (pair it with a
  no-retry-on-401 read policy in the QueryClient's `defaultOptions`).
- **The same door at boot: the gated bootstrap.** An F5 drops the in-memory bearer while the cookie survives, so
  the app root runs `useSession(session.bootstrapSession)` and gates the navigator on `ready`: one deliberate
  rotation before any route fires an authed request. The interceptor and the boot share the seam's single-flight,
  so the two never rotate in parallel.
- **Never two doors.** A bootstrap probe in the session seam *and* a hand-rolled refresh in the client both
  fire on a cold load — two parallel rotations, one burned family. A pilot shipped each half in the
  same week from different branches; the merge is where the race was caught. `SKYFE029` closes the
  door mechanically: the refresh hook/operation (and any hand-rolled POST to a refresh route) is
  consumable only inside `lib/skies-client` / `lib/session`.

---

## Sign-in is an identity change, not a rotation — the seam's two resets

The session seam (`lib/session`, `createSessionSeam`) writes the token through one door (`SKYFE016`),
pairing the write with a cache reset so a just-authenticated user is never bounced by a stale `me`.
But **not every token write is the same kind of write**, and conflating the two is its own prod bug:

- **Rotation** — the *same* identity gets a fresh token (a boot `bootstrapSession`, a 401-refresh).
  The cache is still that user's; only the session-shaped queries (`me`) need re-reading. Light reset,
  screen stays warm. → `onSessionChanged` (e.g. `resetQueries({ queryKey: getMeQueryKey() })`).
- **Identity change** — a *different* user may now hold the session: an explicit `signIn` (sign-in /
  sign-up) or a `clearSession` (sign-out). The prior user's entire cache must be **wiped**, or it bleeds
  into the next session. → `onIdentityChanged` (e.g. `queryClient.clear()`).

This is the **hostpoint** root cause, finally named. A sign-out→sign-in on one client (user A → user B)
was treated as a rotation — only `me` was reset, and the rest of A's cache leaked into B's screens. The
team's "fix" was to **split the app into two** (traveller / host); the real fix is the seam wiping on
identity. With Augusto's httpOnly-cookie + rotation backend (0.4.2) the backend half is done; this is the
front half.

The surface makes the right reset **unskippable by entry point**, so an app cannot authenticate a user
without the wipe:

```ts
export const session = createSessionSeam({
  setAccessToken,
  refresh: () => refresh(), // the refresh cookie rides the request
  onSessionChanged: () => queryClient.resetQueries({ queryKey: getMeQueryKey() }), // rotation — light
  onIdentityChanged: () => queryClient.clear(),                                    // identity — total
});

await session.signIn(loginResult); // identity door → onIdentityChanged (the prior user's cache is gone)
await session.bootstrapSession();  // rotation door → onSessionChanged (same user, warm screen)
await session.clearSession();      // identity door → onIdentityChanged
```

`signIn` is the only authentication entry. `onIdentityChanged` is required: substituting or falling
back to the light `onSessionChanged` reset would recreate the cross-user cache leak. The framework
keeps no compatibility alias or weaker fallback for this security boundary.

---

## Route guards are symmetric — `guardSession`, one primitive both ways

`SKYFE017` polices the *shape* of a guard (branch on a tri-state `SessionState`, never a raw
`isAuthenticated` boolean) — but it cannot catch a guard that is simply **absent**. The **pauta** bug was
exactly that: `/login` and "create account" had *no* guest-guard, so a signed-in user reaching them was
let straight through (and "create account" dropped them into the app). A private-route guard is a reflex;
the guest-guard on the *public* routes is the one apps forget — because each app re-derives both from
scratch.

The spine closes that by making the guard a **pure decision primitive** — `guardSession`, router-agnostic
(it returns data, it never navigates), so the auth-guard and the guest-guard are the **same call with
`allow` flipped**:

```ts
export type GuardOutcome<Href> =
  | { action: "wait" }                       // session still loading → render a splash
  | { action: "render" }                     // allowed → render the route
  | { action: "redirect"; to: Href };        // rejected → send them to redirectTo

guardSession(session, { allow: "authenticated", redirectTo: "/login" }); // private route
guardSession(session, { allow: "anonymous", redirectTo: "/home" });      // public/guest route
```

`loading → wait` (the bounce-to-login case `SessionState` exists to make unspellable), allowed → `render`,
rejected → `redirect`. The app binds it to its router **once**, in a ~10-line component wiring the splash
+ the router's `<Navigate>`, and writes `<AuthRoute>` / `<GuestRoute>` from the same body —
so the guest-guard stops being something to remember and becomes a flag on a shared primitive.

> **No SKYFE rule for the *absent* guard.** A solid "this public auth route has no guest-guard" rule was
> evaluated and **not shipped**: the signal is split across files the single-file linter can't correlate.
> `signIn` is called in the login *ViewModel* (the data door), while the guest-guard lives in the route's
> *layout* (a pathless auth layout route) — two files away, and the idiomatic layout-guard placement means a
> per-file rule flagging the login screen for "not self-wrapping" would false-positive every correctly
> guarded app. `SKYFE018` works only because its param read and its redirect are co-located in one route
> file; this isn't. Forcing the heuristic would trade the framework's near-zero-false-positive bar for
> noise — so the primitive + this convention carry it, not a rule.

---

## Session resilience — the four ways an auth session breaks, and the spine's answer

Auth is treacherous on the read path too: a session expires, a network flaps on an F5, a cold load
double-fires, a role gate is forgotten. Four spine primitives, each born from a confirmed pilot failure,
each enforced by the spine contract:

- **A transient `me` failure is not a sign-out.** `toSessionState` folded *every* error into `anonymous`,
  so a `5xx`/timeout on the `me` query during an F5 over a shaky network bounced an authenticated user to
  login (where re-login also failed). Wire the required classifier `isUnauthorized` from the failed request
  (`isUnauthorized: error?.response?.status === 401`): a classified non-auth error ⇒ `loading` (defer, let
  react-query retry, recover to `authenticated`), only a real `401/403` ⇒ `anonymous`. Pair it with the boot
  timeout below so a permanently-down API cannot pin the splash.

- **A hung bootstrap must not pin the splash forever.** `useSession(bootstrap, { timeoutMs })` arms a
  fallback: a boot rotation that never settles (a dead socket on a cold load) would otherwise hold the
  navigator on the splash with no path to login. After `timeoutMs` the gate opens and the guard decides; a
  late success still set its token first. Omitted ⇒ the original wait-forever.

- **A cold load fires one rotation, not two.** `bootstrapSession` is wrapped in `singleFlight`: React
  StrictMode double-invokes effects in dev, and the boot can race the client's 401-interceptor — two refresh
  rotations replay the spent token and the backend's theft-detection burns the whole family (the `SKYFE029`
  hazard, at boot). `singleFlight(fn)` collapses concurrent callers into one in-flight execution (the gate
  reopens on settle, resolve *or* reject). The seam's `bootstrapSession` is wrapped with it, and the client's
  401 interceptor shares that one gate by calling `bootstrapSession` through the injected `setTokenRefresher`
  — the single-flight refresh gate is a spine primitive, registered once, not a per-app hand-roll. Coalesced callers
  share the first call's result, which is exactly right for a rotation (the credential rides the cookie, not an
  argument) — never wrap a per-argument operation with it.

- **A role gate is the same guard, carrying a predicate.** `guardSession`'s `allow` also accepts
  `(user) => boolean` — a capability/role gate (`allow: (u) => u.role === "admin"`). Authorization is a fact
  about the user **data**, not a second identity axis (the invariant: *auth = one identity, authz =
  capability*), so a role-guarded route is one flag on the shared primitive, never a bespoke hand-rolled
  check that the next screen forgets. An anonymous visitor has no user to inspect, so they redirect like any
  rejected visitor — the predicate is never even called.

---

## Endpoint kinds — the wiring vocabulary

Not every endpoint should have a frontend wiring, and that is not a rare exception (webhooks, internal
server-to-server, OAuth redirects, browser-loaded assets). So the framework classifies an endpoint's **nature**
with a closed vocabulary — `WithEndpointKind(EndpointKind.X)` on the slice's `Map` (a builder call from
`Skies.Framework.AspNetCore`, because a minimal-API handler is a lambda that cannot carry a class attribute to the
endpoint). This is **classification, not suppression**: it does not say "ignore the rule here", it says "this
endpoint *is* a webhook", and the harness derives that a webhook has no UI wiring.

- **Opt-out, not opt-in.** The default is `EndpointKind.App` — app-facing — and needs no call. The legitimate
  exception (a webhook) costs one call. This is right *because* a Skies app is UI-first — app-facing is dominant.
  An API-first product would reconsider.
- **One classification, many derivations.** The kind tags the endpoint in OpenAPI (`AddSkiesOpenApi`), and orval's
  audience filter drops the non-app kinds from the client; a future backend doctor rule can hold a `Webhook` to
  verifying its signature / being idempotent. One declaration, several enforcements; intent flowing back→front.
- **Closed enum of natures, zero behavior params** — the guard-rail against the mini-language the constitution
  forbids. The marker says *what it is*; the `Handle` says *what it does*.
  - (no call) → `EndpointKind.App` — app-facing; must be wired.
  - `WithEndpointKind(EndpointKind.Asset)` → a browser-loaded file/image URL carried by another contract; never a
    data operation.
  - `WithEndpointKind(EndpointKind.Webhook)` → third-party callback; never UI.
  - `WithEndpointKind(EndpointKind.Internal)` → server-to-server; outside any client.
  - Multiple app audiences are separate manifest surfaces whose source roots are checked as one product union.
  - Forbidden: a kind that carries configuration (`Webhook(Retries = 3, Signature = "hmac")`) — config-as-annotation
    is the fattening that killed earlier scenarios. Retry/signature/idempotency live in the `Handle`, visibly,
    never in the mark.

---

## Server-driven actions — a closed kind, never a client route

When the backend drives a navigation (a pending-task card, a CTA), the contract carries **which
action** as a closed enum; the client owns **where that goes**. A route string crossing the
server→client boundary (`ctaTarget: "/host/properties/new"`) is the documented anti-pattern: it is
runtime data — invisible to OpenAPI, to `tsc`, and to typed routes — and a pilot shipped it to prod
(two server-minted routes didn't exist in the app → 404 on click; the backend half of this convention
lives in [CONVENTIONS.md](CONVENTIONS.md) §"The contract never mints a client route").

The client-side shape is a `Record` over the **generated** enum, so both failure modes die in the
typecheck:

```ts
import { PendingKind } from "@/client.gen/model"; // the closed enum, generated from the contract

const PENDING_ROUTE: Record<PendingKind, AppRoute> = { // AppRoute: the router's typed path union
  [PendingKind.CompleteListing]: "/host/properties/new",
  [PendingKind.AcceptTerms]: "/onboarding/host/intermediation-terms",
};
// exhaustiveness: a NEW kind breaks this Record until it is mapped (no silent dead card);
// validity: each value is a typed route (typed routes on), so a drifted literal does not compile.
const openPending = (p: Pending) => navigate({ to: PENDING_ROUTE[p.kind] });
```

This composes with `SKYFE030`: typed routes make the `Record`'s values compile-checked, and the
no-cast rule keeps anyone from smuggling a raw server string into `navigate` anyway. **The config
pair matters** — TanStack Router gets typed routes from its generated route tree, React Router from its
generated route types; without typed routes the rule still bans the cast, but the literal degrades to an
unchecked `string`.

---

## The bright line — generate vs scaffold (the law)

This is the whole game. Cross it wrong and the harness becomes the source-gen vector.

| | **Generate** (re-emits every build; you never edit) | **Scaffold** (`g`, runs once; you own it after) |
|---|---|---|
| Contract types (`Input`/`Output` → TS) | ✅ plumbing — the wire | |
| Typed slice-hook (`useDeposit()`) | ✅ plumbing — a wrap of the client | |
| ViewModel body (state, commands, derived, UX) | ❌ **source-gen of behavior — the vector we refuse** | ✅ a visible skeleton you write |

The test that separates them: **scaffold** runs on demand, writes visible code you edit and
own, and is doctor-removable (deleting the generator does not touch existing files) — the output
*is* the source. **Source-gen** runs every build, owns its output, clobbers your edits, and the
behavior lives in the generator, not the file.

`skies g feature <Name>` scaffolds the `view`/`viewModel`/`i18n` unit once, with the **types fiber from the
contract** and the behavior left visible for the application to write — a starting point, never an owner. Tests
are not scaffolded: a feature's cases live in its spec, written against its failure modes. Explicitly **out**, on
the same law: the predecessor's "smart stubs" that pre-filled the body with the "correct" runtime call. That
delegates behavior; it is the frontend twin of the "runtime framework you inherit from" the back rejects.

If the contract changes, the generated `*.gen.ts` regenerates (plumbing) and `tsc` breaks the
ViewModel where it is now wrong — you fix it by hand. The type enforces the drift; you own the
behavior.

---

## The harness — rule catalog (`SKYFE*`)

The frontend doctor is an **ESLint custom plugin** (`@skiesjs/eslint-plugin`) for in-file rules,
run by `npm run lint` and by `skies doctor`. ESLint
is the mature path for custom semantic rules — hostpoint reached for Biome and had to hand-roll
a `.mjs` scanner for exactly this, the tell that Biome's custom plugins are not yet there.

With the MVVM seam, the policed surface collapses to **the ViewModel** — the View is mock-free
by construction, and completeness is the compiler. Every rule is born from observed pain
(hostpoint's port + the predecessor's wiring rules), never speculation. The routing rules
(`SKYFE015`–`019`, `022`, `030`) recognize TanStack Router and React Router idioms; they police a shape, not a
router runtime.

| Rule | Enforces | Status | Origin |
|------|----------|--------|--------|
| `SKYFE001` | View purity — a `*.view.tsx` imports no data layer (generated hooks, the client, `fetch`/`axios`); it consumes its ViewModel. Type-only imports of the contract are exempt | **shipped** | the wired-only seam — keeps the View mock-free |
| `SKYFE002` | ViewModel is the only data door — only `*.viewModel.ts` (plus the auth/routing infra seams, `lib/session`/`lib/guards`) may consume generated operations. Re-exporting them (`export … from "client.gen"`) outside the doors is the laundering bypass, also flagged; contract types and generated enum values stay free | **shipped** | one data path, one policed surface |
| `SKYFE003` | **No mock in production code** — no import from `**/__mocks__`/`**/fixtures`/MSW outside `*.test.*` | **shipped** | hostpoint: `WAR-*` storybook fixtures shipped as data |
| `SKYFE004` | ViewModel is render-agnostic — a `*.viewModel.ts` imports no JSX/`react-dom` | planned | keeps the ViewModel testable without rendering |
| `SKYFE007` | Mandatory states — a ViewModel exposing server data exposes `loading` + `error` + `empty` | planned | visible failures need an explicit state |
| `SKYFE010` | **State completeness** — a `*.view.tsx` routes loading/error/empty through `<Resource>` (the spine), not raw `isPending`/`isError` | **shipped** | every async state handled by construction, not a hand-rolled branch that forgets one |
| `SKYFE011` | **i18n parity** — every locale object in a `*.i18n.ts` declares the same keys, compared as **flattened paths** (`empty.title`) so a key missing inside a nested group is caught too; a key in one language but not its siblings is a silent untranslated string | **shipped** | no string ships untranslated in any language |
| `SKYFE013` | **Mutation surfaces its error** — a react-query `.mutate(...)`/`.mutateAsync(...)` in a ViewModel routes its failure somewhere (inline `onError`, a read `.isError` state, a try/catch or `.catch()` on `mutateAsync`, or a propagated return). An **empty** `onError: () => {}` is flagged too — the silent failure with paperwork. With the `SKYFE027` defaults wired, the app sets `{ globalSurface: true }`: the global `MutationCache.onError` IS the surface (react-query fires it regardless of per-call handlers), so a bare `.mutate()` passes and only the empty handler stays flagged | **shipped** | the front-side of the backend's `error_handling` — no silent failure, no `onError` theater |
| `SKYFE014` | **No hardcoded copy** — user-facing JSX text + copy props (`placeholder`, `label`, `title`, `aria-label`, `alt`…) in a View go through i18n (`t()`), not literals | **shipped** | feeds the catalog that `SKYFE011` then keeps complete |
| `SKYFE015` | **No imperative redirect inside `useEffect`** — a redirect-on-state is declarative (`if (terminal) return <Navigate … />`), never `router.navigate`/a `useNavigate()` call in an effect: it runs after paint and re-fires every render (a flash at best; a navigation/refetch loop when the effect re-triggers a guard's query). Navigation on a user action stays allowed. Scoped to the navigating layer (views + routes) | **shipped** | the pilot shipped this loop twice (Splash, then ChooseRole + 5 screens) before the rule existed |
| `SKYFE016` | **Session one door** — the bearer token is written through one seam (`lib/session`, where the write is paired with a `me`-cache reset); a `*.viewModel`/`*.view` importing the token setter (`setAccessToken`…) directly — **or writing a token-ish key straight to storage** (`localStorage`/`sessionStorage.setItem("…token…", …)`) — is the scattered write that forgets the reset | **shipped** | pauta: a forgotten reset after registration bounced the new user back to `/login` |
| `SKYFE017` | **Guard tri-state** — a route guard redirects on a `SessionState` (`loading \| authenticated \| anonymous`), never a raw `isAuthenticated` boolean (which reads "still loading" as "signed out"). The read-side twin of `SKYFE010` | **shipped** | the bounce-to-login root cause: a boolean collapses the still-loading case |
| `SKYFE018` | **Route param guard** — a route reading a required id param through a loose `useParams()` (React Router's bare call, TanStack's `{ strict: false }`) guards its absence with a declarative redirect, so a param-less hit (bookmark / stale link) can't render a ghost screen on an empty id. The spine's `requiredParam()` union (`missing \| ready`) is the blessed guard shape (`if (id.status === "missing") return <Navigate/>`), recognized beside the bare `!id` form. A strict TanStack read is guaranteed by the matched route | **shipped** | hostpoint: a param-less `/messaging/chat` rendered an empty "ghost" thread |
| `SKYFE019` | **Safe back** — no bare `history.back()` (on `window.history` or TanStack's `router.history`) or React Router `navigate(-1)`; Back goes through a guarded helper (the spine's `safeBack` / an app `useGoBack`) that falls back to a parent when there's no in-app history | **shipped** | hostpoint: deep-linked screens had a dead "Voltar" button (~13 screens migrated) |
| `SKYFE020` | **No hardcoded API base URL** — the base URL comes from configuration (env `VITE_API_URL`, a relative base, or an injected default), never a host baked into `axios.create({ baseURL: "http://…" })`. The backend pins its dev port in `launchSettings`, so the two agree by construction | **shipped** | pauta: the front baked `:8080` while the API ran on the .NET default `:5000` → `me` 404'd → the registered user bounced to login |
| `SKYFE021` | **No raw HTML** — no `dangerouslySetInnerHTML` outside the one audited seam (`lib/html`). JSX escapes by construction; raw HTML is the XSS door, and if the app renders rich HTML (a CMS body) the sanitizer lives in that seam, reviewable | **shipped** | the single React opt-out of escaping must not scatter across screens |
| `SKYFE022` | **No open redirect** — never navigate to a value that arrived in the URL (`navigate({ to: returnTo })` / `location.href = next` off `useSearch`/`useSearchParams`); map the param through an **allowlist** of known in-app routes first | **shipped** | the phishing primitive: a crafted link sends the session-carrying browser anywhere the attacker chose |
| `SKYFE023` | No orphan placeholder — `// wire later`, `TODO`/`FIXME`, `WAR-*`, or `@ts-expect-error` on a data call | planned | mirror of `SKYSELF002` — "almost done" is not done (renumbered as shipped rules claimed the lower slots) |
| `SKYFE027` | **QueryClient carries the mutation defaults** — every production `new QueryClient(...)` wires `mutationCache: new MutationCache({ onSuccess, onError })`: success invalidates every active query + posts the success note (`meta.silent` opts out of the note), failure routes through the feedback seam unconditionally. Tests and the shared test harness (`test/`, `test-utils/`) build bare clients freely. Scaffolded as `lib/query.ts` | **shipped** | pauta: a created category only appeared after F5, with no toast — 13 of 43 ViewModels had no invalidation at all |
| `SKYFE028` | **No manual refetch ritual** — an `onSuccess` whose entire body is refetch/invalidate calls (inline, named, or `useCallback`-wrapped) duplicates the `SKYFE027` defaults; delete it. A handler that does *more* than refetch (navigate, reset, hand off an id) is behavior — never flagged. Warn-tier: reveals, does not gate | **shipped** | pauta: 30 of 43 ViewModels hand-rolled `onSuccess: refetch` — the convention the majority groped toward, pinned so the minority can't forget it |
| `SKYFE029` | **Refresh one-door** — the refresh hook/operation (and any hand-rolled `POST` to a refresh route) is consumed only inside the rotation doors (`lib/skies-client`, `lib/session`); anywhere else is a second rotation path. Type-only imports stay free | **shipped** | pauta near-miss: a session-seam refresh bootstrap and a client 401 interceptor landed the same week from different branches — two cold-load rotations would have tripped the backend's theft detection and burned the session family |
| `SKYFE030` | **No cast on a navigation target** — no `as never`/`as any`/`as unknown` on the argument of `router.navigate` (or a `useNavigate()` call), nor on the `to` of `<Navigate>`/`<Link>`. The cast exists to silence typed routes; silenced, a drifted route literal compiles clean and 404s in prod. Pass a typed literal or the `{ to, params }` shape. **Config pair**: typed routes ON (TanStack's route tree / React Router's route types) — without it the removed cast merely degrades to `string`. Error-tier, routing family | **shipped** | hostpoint: ~8 call sites cast their navigation target to `never`; when the backend minted two routes that didn't exist (the sibling convention), the muted router compiled them clean → prod 404 |
| `SKYFE031` | **Submit handles the invalid path** — in a `*.viewModel.ts`, a one-argument `handleSubmit(onValid)` is flagged: a validation failure runs no code (it happens *before* the mutation, so `SKYFE013`/`SKYFE027` never see it). Use the spine's `submitOrReveal(form.handleSubmit, onValid, { onInvalid })` — it forces the surface and resolves the first invalid field for the shell to navigate to — or pass `onInvalid` by hand. Warn-tier on entry (a single-screen form with visible inline errors is legitimate); promotes with `SKYFE032` | **shipped** | hostpoint: a 9-tab property editor's Save went completely mute when a hidden tab's field failed — no mutation, no toast, no error ("não está salvando a propriedade", in prod) |
| `SKYFE032` | **Controller surfaces its fieldState** — a `<Controller>` whose inline `render` never reads `fieldState` (destructured or accessed) leaves that field's validation error with no surface; pass `error={fieldState.error?.message}` to the field component. Near-zero false positives (`error` on an unvalidated field is inert); a deliberately non-inline surface must still expose the same error state explicitly. Warn-tier, promoted together with `SKYFE031` — the pair makes "a validation error always shows" hold by construction | **shipped** | hostpoint: the Description input destructured only `{ field }` — its validation failure had no surface at all (same incident as SKYFE031) |
| `SKYFE036` | **Tests live in a spec** — a call to `test`/`it`/`describe` (or a member such as `test.describe`, `it.each([…])(…)`, `describe.skip`) imported from `vitest`, `@playwright/test`, `@jest/globals`, or `bun:test`, or used as a global, is flagged in any file whose path has no `.specs/` segment. Reported once per file. A local function that merely shares the name is not a test. It asks where a test lives, never that one exists | **shipped** | tests written as coverage after the code guarded nothing and proved nothing; the spec is the only place a test is tied to a failure mode and a red-then-green receipt |

The gaps in the numbering are rules Skies 5 removed. The latest is `SKYFE009`, which kept ViewModels free of React
Native imports so web and native could share them; it went with the React Native track.

The two directions are asymmetric, and that sets the severity: **front→back** (the UI calls an
endpoint that doesn't exist) is never valid → a hard **error**, free from `tsc` (the hook isn't
generated, so it can't compile). **back→front** (the endpoint exists, nothing wired it yet) is a
legitimate intermediate state, not an error. "Does this call a real endpoint?" is **not** a
rule. It is `tsc` against the generated client. Lean on the type system; the harness only forbids the
bypass.

**Contract freshness.** Regenerate the client with `skies g client` whenever the backend contract moves; the
generated code is committed, so a stale mirror shows up as a diff in review and as a type error at the call site.

---

## Specs — every test lives in one

Frontend features are accepted the same way as backend ones: a spec folder under `.specs/` with its failure
modes, cases in `e2e/`, and a receipt from `skies proof record` (see [CONVENTIONS.md](CONVENTIONS.md#specs-and-proofs)).
The engine is the app's: Playwright for the real browser, Vitest for a screen's View + ViewModel in jsdom. Declare a
runner in `Skies.toml` that runs one spec folder and writes a JUnit report, and name each case after the failure
mode it covers (`test("FM-3: an expired session lands on sign-in")`). Nothing in the ViewModel, the View, or a JSON
manifest points at the spec.

**A spec is the only home for a test** (`SKYFE036`). There is no `Foo.test.tsx` beside `Foo.viewModel.ts`: a test
written as coverage guards nothing it names and never proved it can fail. An isolated system (a formatter, a
reducer, a UI kit) gets its own spec whose `e2e/` holds isolated cases; the folder name means "the spec's cases",
not strictly end-to-end. Cases import the code they exercise by a relative path or an alias. The sample runs its
web specs with Vitest's JUnit reporter, the spec folder as the filter:

```toml
[runners.web]
setup = "test -d ../../frontend-sdk/node_modules || npm --prefix ../../frontend-sdk ci --prefer-offline"
command = "node ../../frontend-sdk/node_modules/vitest/vitest.mjs run --config ../../frontend-sdk/vitest.config.ts --reporter=junit --outputFile={report} {dir}"
```

The `setup` matters: `skies proof record` runs red in a fresh git worktree, which has no `node_modules`. In an app,
point the paths at the package (`npm --prefix clients/web ci`, `clients/web/node_modules/vitest/vitest.mjs`) and
include `.specs/*/e2e/**/*.test.{ts,tsx}` in the Vitest config, the tsconfig the cases compile under, and the
ESLint `files`.

Every runner gets `SKIES_EVIDENCE` and `SKIES_SPEC` in its environment; a test writes artifacts to
`process.env.SKIES_EVIDENCE` when it is set, and they become the spec's hashed `evidence/`. When an Assay archetype
decides a failure mode, tag its spec.md line (`- FM-4 … [avp: <criterion-id>]`) and have the case write the verdict
with `verdictToJsonLine(verdict)` to `$SKIES_EVIDENCE/avp-FM-4.json`: the mode then passes only with a passing
verdict. The tag is optional.

---

## Code comments — the code speaks for itself

Comments are **English**, and they earn their place. The default is **no comment**: a well-named
ViewModel/hook/component + the types say what the code does. A comment exists only to say what the
code *can't* — a non-obvious **why**, a **gotcha**, an invariant, a contract quirk. Rails-style:
prose that adds signal, never restating the line below it.

Explicitly **out** (this is the junk an LLM tends to emit — strip it on sight):
- Migration play-by-play / thinking-out-loud (`// Faithful clone of the old screen…`, `// re-skinned
  onto…`, `// Step 1: …`). The git history is the narrative; the file is not.
- Restating the obvious (`// the name field` over `name:`, `// loading state` over `loading`).
- Mixed PT/EN. Comments are English; **only user-facing copy is pt-BR, and that lives in i18n**, not
  in comments or string literals.

Keep: a non-obvious gotcha (e.g. *why* a value is coerced, *why* an effect is gated), a contract
caveat, an invariant. If you're unsure whether a comment earns its place, delete it.

## i18n — react-i18next, per-feature namespaces

User-facing copy is **never inlined** in a View; it goes through `react-i18next`. One i18next instance
(`src/i18n`), pt-BR today. Each feature owns a **namespace** = its folder name, in a co-located
`src/features/<feat>/<feat>.i18n.ts` (the `ptBR` export), assembled by `skies i18n` into
`src/i18n/resources.generated.ts`; shared copy (nav, generic actions) lives in the `common` namespace. A View reads `const { t } =
useTranslation("<feat>")` and renders `t("some.key")`. Adding a locale is a second key in `resources`
+ a language switch — the feature namespaces don't change. (Like styling, i18n is the app's choice, not
a framework mechanism; this is Hostpoint's.)

**Error codes — translated in every language, enforced.** The backend ships every error as a stable code
(`ErrorBody.code`, the registry constants behind `SKY0018`/`SKY0019`); the front owns the copy in its
`api-errors` catalog, typed against the generated `ErrorBody.code` union so a missing code is a type error, and
`SKYFE011` keeps every locale in step. Composed: code → copy → in every language. This is the front end of the same full-stack discipline `SKY0018`/`SKY0019` enforce on the back.

## Accessibility — jsx-a11y beside the SKYFE plugin

a11y is part of the harness, but it is not architecture, so it is not a SKYFE rule: the DOM speaks `alt`,
`aria-*`, and `href`, and [`eslint-plugin-jsx-a11y`](https://www.npmjs.com/package/eslint-plugin-jsx-a11y) (its
recommended set) polices them, wired in the ESLint config *alongside* the SKYFE plugin, never reinvented inside
it. The same posture as the curated community kit (`sonarjs`, `no-secrets`, `@tanstack/query`). A new app can
start it at warn and promote per rule once the revealed backlog is cleared; the sample runs it at error. Flutter's
accessibility checks are in [FLUTTER-CONVENTIONS.md](FLUTTER-CONVENTIONS.md).

## Scope — and non-goals

**In:** the MVVM feature convention, the `SKYFE*` rules, `skies g feature`, and `skies g client` (stock
orval, wrapped) with the shipped config + mutator. One blessed frontend shape for the web.

**Out (non-goals), by decision:**
- **No bespoke generator.** orval stock, wrapped — never a Skies OpenAPI→TS compiler. (The
  bespoke-compiler vector.)
- **No source-gen of behavior.** The ViewModel body is scaffolded once and owned, never
  re-emitted. No "smart stubs" that pre-fill logic. (The source-gen vector.)
- **No MVVM framework.** Plain custom hooks, not classes/observables/two-way binding. (The
  stranger-maintainable law.)
- **No design system in the framework.** Styling library, component kit, tokens, spacing scale, and layout
  rules are the app's. Every product has its own design language; a framework-imposed vocabulary only added
  rules to fight. (Skies 4 shipped a token taxonomy and a design lint band; Skies 5 removed both.)
- **No React Native.** Mobile is Flutter. Skies 5 dropped the React Native / Expo track, and with it the shared
  platform-agnostic core that existed only to share ViewModels between web and native.
- **No TS decorators (`@Slice`/`@Risk`).** The backend's `[Slice]` is a first-class
  C# attribute the Roslyn doctor reads natively; React function components have no idiomatic decorator
  seam, and bolting one on (babel `experimentalDecorators`, wrapper indirection) *adds* LLM decision
  space — the opposite of the goal. Symmetry of **concept** (the slice), not of **mechanism**: on the
  front the **folder/file convention is the annotation**, discovered structurally
  (`features/<x>/<X>.view.tsx`), exactly as `[Slice]` is on the back.
- **No multi-app sprawl.** One frontend shape, enforced — sprawl was aerocoding's *N* apps, not
  one blessed convention.
- **No frontend in core.** The harness ships as a separate, optional, doctor-removable package —
  the `skies`/`skies-dev` split, applied again. It never enters `Skies.Framework.Abstractions` or
  `Skies.Framework.Doctor`.

When a proposal smells like capability instead of convention + enforcement, it is a scope
violation. Reject in line.
