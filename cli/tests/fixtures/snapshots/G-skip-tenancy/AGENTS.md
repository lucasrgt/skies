# Golden — Operating manual for AI agents

This is a **Skies** app: a vertical-slice .NET backend, optional MVVM React or Flutter clients, architecture
doctors that check both, and one `skies` CLI. The conventions exist so an agent has **less to decide** and what it
writes is **checked**. Same mindset as Rails (convention over configuration, semantic density) in plain, idiomatic
C#, TypeScript, and Dart.

---

## The three laws (never violate)

1. **Stranger-maintainable.** Output is always plain, idiomatic code that a developer who has never heard of
   Skies can read and maintain.
2. **Doctor-removable.** Remove the analyzers or the ESLint plugin and the app still **compiles and runs** — you
   only lose enforcement.
3. **Evidence over apparatus.** A feature is done when its spec folder holds a receipt: failure modes written
   first, black-box E2E, every failure mode failing before the change and passing after. No tags, manifests, or
   gates.

---

## How to deliver a feature

Follow the `skies-sdd` skill (`.claude/skills/skies-sdd/SKILL.md`):

1. `skies spec new <slug>` and write the failure modes in `spec.md`. **Stop and show them to the human.**
2. Write the E2E in the spec's `e2e/` folder (namespace `Specs.S<id>`, cases titled `FM-n: …`). Watch them fail.
3. Generate the shapes (`skies g module|slice|entity|vo|crud|hub`) and implement. Keep `skies doctor` clean.
4. `skies proof record <id>` and report the receipt.

**Every test lives in a spec**, and nowhere else: never a unit test written after the code to cover it, never a
test file beside the code. An isolated system (a value object, a calculation, a parser) gets its own spec whose
`e2e/` holds isolated cases, each titled after the failure mode it covers. The doctor flags a test outside
`.specs/` (`SKY0029`, `SKYFE036`, `SKYFL036`).

---

## This repo

- **Backend** `src/Golden.Api` — .NET vertical slices; the `SKY*` Roslyn analyzers run in its
  build.
- **Tests** `tests/Golden.Tests` — boots the real app (`SkiesWebTest<Program>`) and compiles the
  spec cases under `.specs/*/e2e`, and nothing else.
- **Specs** `.specs/` — one folder per feature.
- `Skies.toml` — the topology `skies doctor` checks and the runners `skies proof` uses.

---

## Backend — the vertical slice (the .NET API, checked by `SKY*`)

One feature = **one file** (maximal locality: read the whole feature in one read). The canonical shape
(`SKY0001`):

```csharp
[Slice]                                    // pure marker; module derived from the namespace
public static class Deposit
{
    public record Input(/* … */);          // contract in  — visible
    public record Output(/* … */);         // contract out — visible

    public static async Task<Result<Output>> Handle(Input input, AppDb db, CancellationToken ct)
    {
        // DbContext direct. Behavior lives here, never hidden. No repository, no unit-of-work, no mapper.
    }

    public static RouteHandlerBuilder Map(IEndpointRouteBuilder app) => /* thin */ .WithName(nameof(Deposit));
}
```

- **DbContext direct** — no `IRepository` / unit-of-work / mapper profile (`SKY0006`). The endpoint stays thin,
  an expression-bodied handler, never a statement block (`SKY0002`). Raw SQL never splices runtime values as
  text — a `*Raw` EF call with interpolation/concat is flagged (`SKY0024`); use `FromSql`/`ExecuteSql`.
- **Security is a decision, never an omission**: every slice's endpoint declares its authorization —
  `.RequireAuthorization(…)` or an explicit `.AllowAnonymous()`, on its own `Map` chain or the module's route
  group (`SKY0022`); a `Handle` that injects `ICurrentUser` and never reads it is a missing ownership check
  (`SKY0023`). A curated CA* security floor (dropped `CancellationToken`, insecure deserialization, broken
  crypto/TLS) ships with the doctor at error tier (opt-out: `<SkiesSecurityAnalysis>false</…>`).
- **Modules** own both halves of their wiring — `AddServices` + `Map` (`SKY0015/16`); `Program.cs` is only an
  index (`SKY0017`). Each module carries a `<Module>.ctx.md` (`## Boundaries` + `## Design notes`, non-empty and
  kept **fresh** — `SKY0004/05`).
- **Domain is always-valid**: a `[ValueObject]`/`[Entity]` exposes no public constructor or setter and is built
  only through a smart constructor returning `Result<T>` (`SKY0013/14`); a persisted or entity-owned type must
  declare its mark (`SKY0021`). **Write-ownership**: a module writes only its own entities — on a `DbSet` or
  through the untyped `db.Add(entity)` (`SKY0009`). A held `Result<T>` is **checked before unwrapped** —
  `IsSuccess`/`IsFailure`/`Validation.Collect` before `.Value`/`.Error` (`SKY0025`). Every persisted write's
  entity carries a concurrency token (`[Timestamp] RowVersion` — `SKY0026`, warn-tier).
- **Validation inline** at the top of `Handle`, accumulated with `Validation` — `Check`/`Collect` plus the
  shorthands `Require(guid, field, code)`, `NotBlank`, `InRange`.
- **Errors are registry constants** on a `*ErrorCodes` class (`SKY0018/19`) — the OpenAPI + i18n seam.
  `.WithName(nameof(Slice))` (`SKY0012`) is what the typed client turns into the `use<Slice>` hook.
- Files ≤ 500 LOC (`SKY0007`). A test method outside `.specs/` is flagged (`SKY0029`).

---

## Frontend — MVVM + the spine (the React clients, checked by `SKYFE*`)

A screen is a pair: a **View** that renders and a **ViewModel** that owns data. Styling and components are the
app's own.

- **View renders only** (`SKYFE001`); the **ViewModel is the one data door** to the generated client (`SKYFE002`)
  and is **platform-agnostic** — no `react-native`/`expo` (`SKYFE009`), so the core is shared web↔mobile.
- Async state flows through the spine's `AsyncState` + `<Resource>` (`SKYFE010`), never raw `isPending`/`isError`
  (multi-query screens fold with `combineAsyncStates`). Mutations surface their error — and an empty
  `onError: () => {}` doesn't count (`SKYFE013`). No mocks in production (`SKYFE003`). Copy goes through i18n,
  with locale-key parity checked as flattened nested paths (`SKYFE011/14`). The API base URL comes from config,
  never a hardcoded host (`SKYFE020`).
- **Security** (`SKYFE021–022`): no `dangerouslySetInnerHTML` outside the one audited `lib/html` seam (the XSS
  door); never navigate to a value that arrived in the URL — allowlist it first (open redirect).
- **Routing & session** (`SKYFE015–019`, `SKYFE030`) — the navigation harness, born from real pilot bugs and
  router-agnostic (recognizes expo-router ↔ TanStack):
  - **`SKYFE015`** — a redirect-on-state is declarative (`return <Redirect/Navigate …/>`), never `router.replace`
    / `router.navigate` / a `useNavigate()` call inside a `useEffect`.
  - **`SKYFE016`** — the bearer token is written through **one seam** (`lib/session`) that pairs the write with a
    `me`-cache **reset**; a scattered write — importing the setter directly **or** writing a token-ish key to
    `localStorage`/`AsyncStorage`/`SecureStore` — forgets the reset and bounces a just-authenticated user to login.
  - **`SKYFE017`** — a guard branches on a **tri-state** `SessionState` (`loading | authenticated | anonymous`),
    never an `isAuthenticated` boolean (which reads "still loading" as "signed out").
  - **`SKYFE018`** — a route reading a required id param guards its absence with a declarative redirect (no ghost
    screen on an empty id); the spine's `requiredParam()` union (`missing | ready`) is the blessed guard shape.
  - **`SKYFE019`** — no bare `router.back()`/`history.back()`; Back goes through a guarded helper
    (`safeBack`/`useGoBack`) that falls back to a parent when there is no in-app history.
  - **`SKYFE030`** — no `as never`/`as any`/`as unknown` on a navigation target (a `router.push`/`replace`/
    `navigate` argument, a `useNavigate()` call, or the `href`/`to` of `<Redirect>`/`<Navigate>`/`<Link>`).
    The cast silences typed routes; silenced, a drifted route literal compiles clean and 404s in prod. Keep
    typed routes ON (expo-router `experiments.typedRoutes` / TanStack's route tree) — the rule's config pair.
  - When the **backend drives a navigation** (a pending card, a CTA), the contract carries a **closed kind
    enum**, never a route string — the client owns the `Record<Kind, Href>` map over the generated enum, so a
    new kind is a compile error until mapped and every target is a typed route.
- **Forms & validation** (`SKYFE031–032`, warn-tier) — a validation failure always has a surface:
  - **`SKYFE031`** — a one-argument `handleSubmit(onValid)` in a ViewModel swallows validation failures (the
    mute submit button: the failure happens *before* the mutation, so `SKYFE013/027` never see it). Use the
    spine's `submitOrReveal(form.handleSubmit, onValid, { onInvalid })` — it forces the surface and resolves
    the first invalid field so a tab/step shell can navigate to it — or pass `onInvalid` by hand.
  - **`SKYFE032`** — a `<Controller>` render must read `fieldState` and surface the field's error
    (`error={fieldState.error?.message}`); a render that only takes `{ field }` leaves the error invisible.
  - The spine `@skiesjs/react` ships the primitives these steer toward: `SessionState`/`toSessionState`,
    `AsyncState`/`Resource`/`combineAsyncStates`, `safeBack`, `requiredParam`, `submitOrReveal`.
- **Contract freshness** — regenerate the typed client with `skies g client` whenever the backend contract moves.

Routing rules are **error**-tier (correctness), beside the architecture rules — not the warn-tier polish rules.
A badly-wired route **fails the build**.

---

## Build & verify

```
skies doctor                 # dotnet build (SKY*), eslint (SKYFE*), Flutter rules (SKYFL*)
dotnet test                  # every spec case (the only tests there are)
skies proof status           # which receipts went stale (hashes only)
skies proof verify --stale   # rerun them when your change could affect them
```

Nothing runs automatically. If the doctor is red, **fix the code — never suppress a rule.** A rule fires on a real
defect class; the fix *is* the convention.

---

## The boundary (anti-drift — the Rails posture)

The framework ships the **skeleton + enforcement**; this app brings its own **libraries** (a hashing lib, a
payment SDK, a maps client) and its **business logic**, in plain code. No source-gen of behavior, no vendor
adapters in core, no runtime you inherit from. When a need smells like *capability* rather than
*convention + enforcement*, it lives in the app — not the framework.

---

## The package-first law (anti-desync)

This app consumes the framework **only as versioned NuGet/npm packages** — never as source copies. If a need
here is framework-shaped (a rule, a spine primitive, a harness
mechanism, anything another Skies app would also want), it does **not** get implemented in this repo:
it lands in **Skies first**, ships through the package feed, and arrives here as a version bump whose
doctor fallout you then fix. Writing it here "for now" is how framework code gets lost in time.

App-specific code (your domain, your vendors, your copy, your design system) stays here, obviously — the law is
about *generic* mechanisms only.

---

## Git discipline

- Stage specific files (`git add <path>`), never `-A`/`.`. One commit per concern; lowercase, present-tense
  imperative messages.
- Workspace green every commit. No `--force`, no history rewrites.

---

## Canonical conventions (the full constitution)

This file is the distilled operating manual. The complete catalog + rationale lives in the **Skies**
framework repo: `docs/CONVENTIONS.md` (backend), `docs/FRONTEND-CONVENTIONS.md` (React), and
`docs/FLUTTER-CONVENTIONS.md` (Flutter). Ground every convention fact there, never memory.
