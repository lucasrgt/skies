# Skies.Framework.Starter — Operating manual for AI agents

This is a **Skies (.NET)** app: an opinionated convention bundle — a vertical-slice .NET backend + an
MVVM React frontend + a build-time harness (the *doctor*) that enforces both. The conventions exist so an
agent has **less to decide** and what it writes is **checked**. Same mindset as Rails (convention over
configuration, semantic density) in plain, idiomatic C# + TypeScript — no DSL, no runtime you inherit from.

> Mirrored verbatim at `AGENTS.md` for tooling that loads it (Codex, Aider, …).

---

## The two laws (never violate)

1. **Stranger-maintainable.** Output is always plain, idiomatic C#/TypeScript that a dev who has never heard
   of Skies can read and maintain.
2. **Doctor-removable.** Remove the analyzers / the ESLint plugin and the app still **compiles and runs** —
   you only lose enforcement. The harness is wire, not apparatus.

The goal is **not "less code"** — it is **semantic density**: more meaning per token, in standardized shapes
the doctor can check.

---

## This repo

Topology is declared in `Skies.toml` (the single source of truth; `skies doctor` validates it):

- **Backend** `src/Skies.Framework.Starter.Api` — .NET vertical slices; the `SKY*` Roslyn analyzers gate
  `Skies.Framework.Starter.slnx`.
- **Frontend** — add a React client under `clients/` (`skies g`); the published
  `@skiesjs/eslint-plugin` `SKYFE*` harness gates it.
- **Design system** `.design/contract.toml` — Assay Design makes Atomic Design tiers, component composition,
  tokens, semantics, and surface coverage executable; `npm run design:doctor` validates the contract from birth.
  Before editing UI, agents run `npm run design:recall` (see the `skies-design` skill).
- **Doctor**: `skies doctor` runs both legs.
- **Done-gate**: `skies gate --affected` (doctor + the Git-derived transitive proof closure + the universal
  traceability inventory) — wired from birth into CI and echoed locally by lefthook. `skies gate --full` is the
  exhaustive release audit. A change is done when its affected gate is green; never release without a full green.

---

## Backend — the vertical slice (the .NET API, gated by `SKY*`)

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
- **Tests**: every slice declares a `.spec.toml` criterion and a subject-bound executable `[AVP]`
  (`SKY0030/31/32`). Every shape-derived write has happy **and** sad `[Journey]` proofs whose terminal state is
  asserted (`SKY0008/10/20`). Unknown shapes fail closed as writes. Files ≤ 500 LOC (`SKY0007`).

---

## Frontend — MVVM + the spine (the React clients, gated by `SKYFE*`)

A screen is a triple: a **View** that renders, a **ViewModel** that owns data, a test that mounts it.

The UI also declares its language-neutral design grammar in `.design/contract.toml`: atoms compose into
molecules, organisms, and templates; product pages are verified as named surfaces. Rendered components expose
`data-ui-*` evidence so Assay Design can check the same contract in tests, Storybook, and Figma projections.

- **View renders only** (`SKYFE001`); the **ViewModel is the one data door** to the generated client (`SKYFE002`)
  and is **platform-agnostic** — no `react-native`/`expo` (`SKYFE009`), so the core is shared web↔mobile.
- Async state flows through the spine's `AsyncState` + `<Resource>` (`SKYFE010`), never raw `isPending`/`isError`
  (multi-query screens fold with `combineAsyncStates`). Mutations surface their error — and an empty
  `onError: () => {}` doesn't count (`SKYFE013`). No mocks in production (`SKYFE003`); co-located unit +
  integration tests (`SKYFE005/06`). Copy goes through i18n, with locale-key parity checked as flattened nested
  paths (`SKYFE011/14`); color through design tokens (`SKYFE012`). The API base URL comes from config, never a
  hardcoded host (`SKYFE020`).
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
- **Contract freshness** — the typed client is pinned to the spec it was generated from: the codegen tail stamps
  `client.gen/.spec-hash` and the doctor compares it against the live OpenAPI document. A moved contract is a
  build-time "regenerate", never a runtime 404.
- **Feature proof is complete**: every ViewModel has a co-located Assay proof (`SKYFE033`) and at least two
  subject-bound `@e2e` obligations resolving to executable happy and sad surface flows (`SKYFE035`). No annotation,
  manifest mode, skipped test, or undeclared frontend package can lower that bar.

Routing rules are **error**-tier (correctness), beside the architecture rules — not the warn-tier polish rules.
A badly-wired route **fails the build**.

---

## Build & verify — green before you are done

```
skies gate --staged --fast           # pre-commit: mapped proofs run; exhaustive/browser work waits for CI
skies gate --affected --base <rev> --fast # local pre-push: bounded feedback over the commits being sent
skies gate --affected --base <rev>   # PR CI: every transitively affected backend/frontend proof
skies gate --full                    # release: exhaustive audit
```

Every mode validates the complete proof inventory. The application cannot supply a risk label or test filter;
ambiguous/shared runtime changes widen to a full surface; CLI pins, hooks, and workflows stay doctor-validated
control-plane changes. Unselected rows are `not-affected`, never counterfeit passes.

Never leave the workspace red. If the doctor is red, **fix the code — never suppress a rule.** A rule fires on a
real defect class; the fix *is* the convention.

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

Enforcement: declare the framework checkout in `Skies.toml` (`[framework] repo = "…"`) and `skies doctor`
fails on stale backend/frontend package versions or a retired in-repo copy. App-specific code (your domain,
your vendors, your copy) stays here, obviously — the law is
about *generic* mechanisms only.

---

## Git discipline

- Stage specific files (`git add <path>`), never `-A`/`.`. One commit per concern; lowercase, present-tense
  imperative messages.
- Workspace green every commit. No `--force`, no history rewrites to escape a failing hook — fix forward.

---

## Canonical conventions (the full constitution)

This file is the distilled operating manual. The complete catalog + rationale lives in the **Skies**
framework repo: `docs/CONVENTIONS.md` (backend) and `docs/FRONTEND-CONVENTIONS.md` (frontend). Ground every
convention fact there, never memory.

<!-- skies:foundations:start -->
## Skies foundation workflow

The primary coding agent owns the complete foundation lifecycle. Never create or
delegate one agent per foundation.

1. At task start, run `dotnet tool run skies context --task "<goal>" --path <expected-path>`.
   Treat every returned decision, invariant, way, scar, and due deferment as governing context.
2. Rerun `dotnet tool run skies context` after scope changes, context compaction, or movement into
   an unfamiliar area. Keep retrieval bounded with accurate task text and paths.
3. Use the repository-local foundation skills only when a real lifecycle event occurs: accepted
   decisions for WTW, proven patterns for RTW, corrected failures for NYA, or evidence-backed
   conditional deferments for NWC. Never record hypothetical guidance.
4. Run focused repository tests and linters during implementation.
5. Before commit, stage the exact intended paths. The checked pre-commit hook runs
   `dotnet tool run skies check --task "<completed work>" --staged`; invoke it manually only when validating
   without committing. Staged checks remain bounded while every directly mapped proof runs.
6. Follow the repository's single checked authority boundary. With CI authority, pre-push is
   `--base <target-revision> --fast` and pull-request CI runs affected verification without `--fast`.
   With local authority, the pre-push hook itself runs `--base <target-revision>` without `--fast`, and no
   pull-request workflow is required. Never configure both as authoritative.
7. Run the repository's explicit `--full` release command at its release boundary, locally or in release
   automation. Bare `skies check --task ...` is intentionally invalid so an ambiguous scope cannot start a
   surprise exhaustive run. Do not report delivery complete until its selected checked boundary is green.
8. Rerun the same check after every fix. Exit code 1 means findings remain. Exit code 2 or greater
   means validation was incomplete. Neither is a pass.

Tests, linters, review, and individual foundation commands do not replace `skies check`.
<!-- skies:foundations:end -->
