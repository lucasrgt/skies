# Skies (.NET) — Conventions

Skies is the **opinionated .NET convention bundle**: a standard vertical-slice architecture, a build-time doctor
that enforces it, and an ai-context discipline, so an LLM has less to decide and what it writes is checked. The
Rails mindset (convention over configuration, semantic density: meaning per token), not the Rails mechanism.

**The boundary.** Skies ships the project shape, the generators, the doctor, and spec receipts. Each app brings its
own libraries and business rules in plain C#. A framework feature must be **generic across projects and proven by
real use**; a vendor adapter or one app's capability lives in the app (absorbing the world killed the predecessors).

## The laws

1. **Stranger-maintainable.** Output is plain, idiomatic C# a .NET developer who never heard of Skies can maintain.
2. **Doctor-removable.** Remove the analyzer and the project still compiles and runs; only enforcement is lost.
3. **Evidence over apparatus.** A feature is accepted by a reproducible receipt in its spec folder. No gate, no
   hook, no check that audits the agent, no rule that demands a test, tag, or manifest.

## The slice convention

One feature = **one file**, so an agent reads the whole feature in one read. Shape enforced by `SKY0001`:

```csharp
[Slice]                                  // pure marker; the module comes from the namespace
public static class Deposit
{
    public record Input(/* ... */);
    public record Output(/* ... */);

    public static async Task<Result<Output>> Handle(Input input, AppDb db, CancellationToken ct)
    {
        // DbContext direct. No repository, no unit of work, no mapper profile.
    }

    public static void Map(IEndpointRouteBuilder app) =>
        app.MapPost("/deposit", async (Input input, AppDb db, CancellationToken ct) =>
                (await Handle(input, db, ct)).ToHttp())
            .WithName(nameof(Deposit));                      // the operationId the typed client hooks from (SKY0012)
}
```

- **DbContext direct.** A repository/unit-of-work layer is indirection with no payoff (`SKY0006`).
- **Handlers are HTTP-agnostic.** They return `Result<T>`; `ToHttp` maps it to a status at the boundary, so a
  handler is testable without a host.
- **Errors carry a code, not copy.** `Error` is `(Kind, Code, Message[, Fields])`: `Kind` is the closed category
  mapped to the HTTP status; `Code` is a stable key the **frontend localizes from** (`<module>.<reason>` such as
  `wallets.insufficient_funds`, `<vo>.<reason>`, `<field>.<reason>`); `Message` is an English developer hint, never
  user copy. Factories require a code (`Error.NotFound(code, message)`, `Validation.Check(ok, field, code,
  message)`); `Collect` inherits the value object's code.
- **A code is a registry constant, never a literal.** Each module owns a `<Module>ErrorCodes` class of
  `const string` codes that every `Error`/`Check`/`FieldError` references (`SKY0018`, unused ones `SKY0019`).
  Why: `AddSkiesOpenApi` enumerates the registries into the `ErrorBody.code` schema and `ToHttp` advertises
  `ErrorBody` everywhere, so the client is typed on the closed set and i18n is checked exhaustive. Platform codes
  live on `PlatformErrorCodes` (the framework ships `platform.rate_limited`, rendered by `RejectAsSkiesError()` on
  ASP.NET's rate limiter; apps add their own), so even a 429 arrives as a localizable `ErrorBody`.
- **Rich types carry the semantics.** `Money`, `Cpf`, `Email` over `decimal`/`string`: the type *is* the rule.
- **A module owns both halves of its wiring**: `[Module]` with `AddServices(IServiceCollection, IConfiguration)`
  and `Map(IEndpointRouteBuilder)` (`SKY0015`). The explicit registry `Modules.cs` (`AddModules` / `MapModules`)
  lists every module (`SKY0016`). No reflection, no discovery.
- **The composition root is three named layers.** `Program.cs` is a thin index: `AddSkies()` + `AddPlatform(config)`
  + `AddModules(config)`, then `UseSkies()` / `UsePlatform()` / `MapModules()` (`SKY0017` flags anything else).
  `AddSkies` is the framework's conventions (OpenAPI, enum-as-name JSON). `AddPlatform` / `UsePlatform` is the
  app's cross-cutting infrastructure (`DbContext`, auth, CORS, shared ports), absent when there is none, split by
  concern into `Platform/<Concern>.cs` partials (Persistence, Security, Observability, Web). A vendor or domain
  service belongs in its module's `AddServices`. Why: the index cannot rot into a dumping ground.
- **Authorization is a decision, never an omission.** Every slice endpoint carries `.RequireAuthorization(…)` or
  `.AllowAnonymous()` on its `Map` chain or its module's route group (`SKY0022`). The caller arrives as an injected
  `ICurrentUser` (claims-based, `Skies.Framework.Auth`) and the slice does its own ownership/role/org check; a
  `Handle` that injects `ICurrentUser` and never reads it is flagged (`SKY0023`).
- **Co-located `<Module>.ctx.md`** carries the business why, one per module (see
  [The ctx.md schema](#the-ctxmd-schema)).
- **LAW: validation is inline at the top of `Handle`, never extracted to a method.** Build the value objects,
  accumulate with `Validation` (`Check` for an inline condition, `Collect` for a value object's verdict, shorthands
  `Require(guid, field, code)`, `NotBlank(text, field, code)`, `InRange(value, min, max, field, code)`), then
  `if (validation.Failed) return validation.ToError();`. If it grows, push rules into value objects; never extract a
  `Validate` method.

### Pagination — the canonical page

A paginated list returns one page shape, so the typed client and the frontend pager hooks recognize "a page".

- **`Page<T>(Items, TotalCount, PageNumber, PageSize)`** in `Skies.Framework.Abstractions`, with the **effective**
  values after server-side clamping. `AddSkiesOpenApi` pins it (four members required, collision-free id) and emits
  every numeric schema as plain `number`, so clients never see `number | string` unions.
- **`ToPageAsync(pageNumber, pageSize, maxPageSize = 100, ct)`** lives in the `Skies.Framework.EntityFrameworkCore`
  satellite (the only runtime package referencing EF Core; not in the `Skies` meta-package). Its receiver is
  **`IOrderedQueryable<T>`**: paging without `OrderBy` does not compile, and that survives doctor removal. Count and
  page run over the same queryable, so a count taken before a tenant filter cannot be written. Filter
  (`AcrossOrgs()`, `Where`), then order, then page.
- **Order by a unique key**: `OrderBy(x => x.Name).ThenBy(x => x.Id)`. Ties without a tiebreaker make rows repeat
  and vanish between pages. `SKY0028` (warning) flags a chain with no primary key (`Id` or `{Entity}Id` on the
  queried entity; a foreign `*Id` does not count); an ordering it cannot read stays silent.
- **Page the ordered entity, project the page in memory.** `.Select(...)` erases `IOrderedQueryable<T>` by design,
  and EF cannot translate an `OrderBy` over a positional-record projection. Page first, then `Page<T>.Select`;
  aggregates join the page's ids afterwards:

  ```csharp
  var wallets = await db.Wallets.OrderBy(w => w.Id).ToPageAsync(input.Page, input.PageSize, MaxPageSize, ct);
  return new Output(wallets.Select(w => new WalletView(w.Id, w.Balance.Amount)));
  ```

- **`Input` stays flat** (`int Page = 1, int PageSize = 20`); `MaxPageSize` is a slice-local const, never payload.
  Aggregates travel by composition (`record Output(Page<ReviewView> Reviews, double AverageRating)`).
- **The count is explicit**: `TotalCount` means a `COUNT(*)` ran. Cursor paging, when needed, is a second primitive
  (`CursorPage<T>`), never merged into this one. The canonical slice is the sample's `ListWallets`.
- `SKY0027` (warning) flags a `DbSet`-rooted query materialized with no `Take`/`ToPageAsync`: fine at ten rows,
  degrading with a tenant's data.
- **When `SKY0027` fires**, one answer per shape:
  1. **The set has a nameable domain bound** → `.Take(N)` with `N` a named const (`MaxQueue`) and a comment saying
     why the set is small. A generous cap you cannot justify is silent truncation, not a fix, so a bare number
     (`.Take(500)`) is flagged too.
  2. **The set accretes with usage** (inbox, agenda, history) → `ToPageAsync`/`Page<T>` even before any paging UI;
     page 1 at a generous size is today's response with an honest contract.
  3. **A write over a set that is not aggregate-scoped** (purge, bulk re-status) → set-based
     `ExecuteUpdateAsync`/`ExecuteDeleteAsync` or a batched job; a `Take` would silently skip part of the write.
  4. **None yet** → leave the warning standing: an open `SKY0027` is a pending decision. Never suppress it.
- **The parent-scope exemption has a blind spot.** `Where(m => m.ChatId == id)` is exempt because most child sets
  are bounded by the parent (steps of one job), but an **accreting child** (one chat's messages) passes silently.
  Precision over recall: a false positive teaches suppression. Page accreting children at design time (rung 2).

### The contract never mints a client route

When a slice's output drives navigation (a pending-task card, a CTA), the backend decides **which action exists**,
never **where the client navigates**. The payload carries a **closed enum** (`PendingKind`, a plain C# enum in the
`Output`); the client maps kind → route over the generated enum (FRONTEND-CONVENTIONS.md, server-driven actions).
A server-minted route string (`CtaTarget = "/host/properties/new"`) is invisible to OpenAPI, `tsc`, and typed
routes, and has shipped 404s to production; with the enum, a new kind breaks the client's exhaustive `Record` until
mapped. General rule: the server speaks **domain vocabulary** (kinds, statuses, codes); the client owns the
**presentation mapping** (route, copy, icon), as with error codes.

No analyzer for this: flagging `"/"`-prefixed strings in `*Target`/`*Route`/`*Href`/`*Path` properties false-fires
on legitimate data (API paths, storage paths, webhook URLs), and the client side is already closed by `SKYFE030`.

## The domain — entities and value objects

Rich, self-validating types; no repositories, base classes, or internal event buses (they fail the laws).

- **Value objects (`[ValueObject]`) encapsulate construction**: immutable, no accessible constructor, setter, or
  init accessor, and a static factory returning `Result<T>` (`Money.From`). `SKY0013` checks this shape, not the
  factory's validation logic. Structs still allow `default(T)`; use a class when that zero state is invalid.
- **Entities (`[Entity]`) encapsulate state**: no accessible constructor, setter, or init accessor (`SKY0014`).
  Factories and domain methods validate their inputs and proposed state before applying mutations. A private
  `EnsureValid` helper can share checks, but is not required: merely naming a method cannot enforce invariants.
  Behavior tests prove rejected transitions leave the entity unchanged.
- **Concurrency posture.** An entity a persisted update or delete touches declares a token: `[Timestamp] public
  byte[]? RowVersion { get; private set; }` or `[ConcurrencyCheck]` on a domain field, so concurrent requests cannot
  silently last-write-win (`SKY0026`, warning; insert-only rows and entities merely read are not reported).
- **Scalar value objects are transparent on the wire.** A scalar VO crossing the API subclasses
  `ScalarJsonConverter<TVo, TPrimitive>` (next to the type, via `[JsonConverter]`): it serializes as its primitive,
  invalid input fails through the smart constructor as a 400, and `AddSkiesOpenApi` mirrors the primitive in the
  schema, so the generated client types it as the primitive, not an empty object.
- **Where behavior lives.** The slice owns orchestration and input validation; the entity and its VOs own invariants
  and state transitions. A change that cannot fail is a `void` method (`Wallet.Deposit`); one that can violate a rule
  returns `Result<T>` (`Wallet.Withdraw` refusing an overdraw).
- **Generated CRUD keeps the split.** `skies g crud <Module> <Entity>` never writes a column from a slice: it adds
  `Open(Guid id, <fields>[, Guid userId][, DateTime now])` and `Update(<fields>[, DateTime now])` to the entity
  (from its `{ get; private set; }` fields, through `EnsureValid`, plus `RowVersion` when missing), keeping any the
  author wrote. Create calls `Open`, Update calls `Update` and saves on success, Delete is a plain `Remove`. Rename
  `Update` to the domain's verb once there is one. The generated update validates a shallow copy before applying
  scalar fields to the original; keep `EnsureValid` free of side effects, including changes to referenced objects.
- **The markers are pure** (like `[Slice]`): no base class, no EF semantics; without the doctor they are inert.
- **The mark is not optional where the type is persisted or owned** (`SKY0021`): a `DbSet<T>` type must be
  `[Entity]`, and a complex member of an `[Entity]` must be `[ValueObject]`. Without it, leaving the mark off skips
  `SKY0013`/`SKY0014` entirely. DTOs, options bags, and unused records are not forced to wear a mark.

## Project layout

```
src/<App>.Api/
  Program.cs                 # thin index: AddSkies + AddPlatform + AddModules (+ the matching Use*/Map*)
  GlobalUsings.cs
  AppDb.cs                   # one DbContext for every module
  Platform.cs                # the app's cross-cutting infra (AddPlatform / UsePlatform), optional
  Platform/<Concern>.cs      #   or one partial per concern: Persistence, Security, Observability, Web
  Modules/Modules.cs         # the module registry: AddModules + MapModules
  Modules/<Module>/          # a bounded context; writes only its own entities
    <Module>Module.cs        #   [Module]: AddServices + Map
    <Module>.ctx.md          #   the module's why
    <Entity>.cs              #   entities at the module root
    Slices/<Name>.cs         #   one slice = one operation
  BuildingBlocks/            # shared value objects (Money, Cpf)
tests/<App>.Tests/           # thin runner: compiles .specs/*/e2e and nothing else, provides TestApp
.specs/<id>-<slug>/          # one feature: spec.md + e2e/ + receipt.json (see Specs and proofs)
```

- **Slices in `Slices/` (one term with `[Slice]`, never `Features/`), domain at the module root**, so ten operations
  never bury the domain. The namespace is `<App>.Api.Modules.<Module>`: folders organize files, the namespace is the
  module.
- **Modular monolith: one `AppDb`, modules are bounded contexts by convention.** Reads, joins, and in-process calls
  across modules are free (a dashboard is one query). A module **writes only its own entities** (`SKY0009`) and
  references other modules **by id, never an EF foreign key**. A cross-module effect goes through the owner's
  service or a job; domain events and an outbox are for genuinely async external integrations (a payment webhook).
  Extracting a module later re-platforms only its cross-module reads.
- `skies new`, `skies g module`, and `skies g slice` produce exactly this shape.

## Real-time — hubs (opt-in)

A new app has no hub; `skies g hub <Module> <Name>` scaffolds one at `Modules/<Module>/Realtime/<Name>Hub.cs`.

- **A hub is wire, not logic.** A hub method persists nothing itself: it calls the matching slice (the one source
  of the write and its rules) and fans the result out. Ephemeral signals (typing, presence) never touch the database.
- **The caller comes from `Context.User` via `ClaimsCurrentUser`**, not the request-scoped `ICurrentUser`: a hub
  runs outside the HTTP pipeline. The token rides the query string on hub paths (WebSockets cannot send an
  `Authorization` header); the generator prints the `Program.cs` wiring.
- **Webhook ≠ hub**: an inbound provider callback is a normal slice. **Ephemeral state is not an entity**: an
  in-memory singleton on one instance; scaling out swaps in a backplane and Redis at the composition root.

## Auth — the mechanism is a package, the policy is the app's

Generated Account modules use `[Module]`, `AddServices`, and `Map` through the same registry as other modules.
`Platform.AddPlatform` owns the database and external providers. The starter's InMemory database, local JWT key,
and fake/console providers are Development-only: startup refuses other environments until the owner replaces
that platform setup with persistent storage, a configured `Jwt:Secret`, and real providers. For OAuth use
`OidcIdTokenVerifier` with the provider authority and client id. Keep local substitutes inside an explicit
Development branch. Generation refuses to overwrite existing owner files before writing anything.

`skies g auth` (and `auth:otp`, `auth:oauth`, `auth:email`) generates the Account module's slices, entities, and
spec, but not the security mechanics: those are `Skies.Framework.Auth` (and the `Identity` port), so a fix reaches
every app through a package version instead of a template no generated app ever sees again. The module registers
them with one explicit call in its composition, `AddSkiesAuth<UserSessionStore>(new SkiesAuthOptions(...))` (plus
`AddVerificationTokens<VerificationTokenStore>()` once a phone or email flow is added).

| Mechanism (package) | Policy and domain (app) |
|---------------------|-------------------------|
| `IPasswordHasher` (argon2id): hash, constant-time verify, the dummy verification that makes "no such account" cost what "wrong password" costs | the password rule (minimum length) and when a password may change |
| `OpaqueTokens`: 256-bit tokens, SHA256 lookup hashes, constant-time `Matches` | nothing: a token is never app-shaped |
| `RefreshSessions`: open a family, rotate, burn the family on reuse, the sliding lifetime and absolute ceiling, revoke by token, family, user, or all-but-current | which error code each `RefreshOutcome` maps to, who may revoke which session (the ownership check), the two lifetimes (`RefreshSessionOptions`) |
| `VerificationTokens`: issue and consume single-use links and 6-digit codes, expiry, purpose binding, the attempt cap that locks a code | each purpose's lifetime, the message that carries the secret, what a verified secret unlocks, the cap (`VerificationOptions`) |
| `RefreshCookie`: httpOnly/Secure/SameSite, the `X-Client: web` switch, `Deliver` (which body leaves, cookie lifetime = session lifetime) | the cookie's name, path, and (for a multi-subdomain app) domain and SameSite; the two response bodies |
| `IAccessTokens`, `ICurrentUser`, the JwtBearer validator, from one secret/issuer/audience | the claims' values: org, role, display name |
| `OidcIdTokenVerifier` (`IExternalIdentityVerifier`): signature against the provider's keys, issuer, audience, lifetime, verified email | which providers the app trusts and their client ids |

- **Persistence stays app-owned.** The app keeps `User`, `UserSession`, `VerificationToken`, its role model, and
  tenancy as ordinary `[Entity]` types in its own `AppDb`. The package reaches them only through two small store
  interfaces the app implements in a few lines of EF each (`IRefreshSessionStore`, `IVerificationStore`): plain data
  access, no decisions. The services re-check whatever a store returns (hash in constant time, purpose, expiry, use),
  so a loose store can make a secret fail, never pass.
- **Slices keep the Skies shape.** `Input`/`Output`/`Handle`/`Map`, the error codes, and the auth posture stay in the
  slice; `Handle` takes the service it needs (`IPasswordHasher`, `RefreshSessions`, `VerificationTokens`) and maps
  each outcome enum to its `AccountErrorCodes` constant. No base class, no generated code, no discovery: the services
  are registered by name and injected like any other.
- **Moving an app generated before this.** `skies migrate 5` never rewrites auth code, which is the app's own; it
  notes a Skies 4 Account module. Regenerate with `skies g auth` in a branch, compare, and port the app's changes.

## The ctx.md schema

Each module carries one `<Module>.ctx.md`: the business *why* the code cannot show. Anything recoverable from the
types, tests, or routes is duplication, and duplication rots.

**Spine, required (`SKY0004`):**

- `# <module>` and a 1–3 line purpose.
- `## Boundaries`: inside, outside, non-goals. Stops scope leak; the highest-value section.
- `## Design notes`: the non-obvious invariants and why they hold. Performance, security, and cross-module effects
  fold in here when they carry a why. Each invariant cites the spec that proves it (below).

**Optional:** `## Wiring` (dependencies not obvious from imports), `## Not yet ported` (deliberate absences).
**Excluded** (recoverable, so it would drift): data models, DTOs, route or error tables, test matrices, examples,
file lists, change logs (decisions go to an ADR and git), diagrams (explain a non-linear flow in prose).

**Spec citations tie the prose to evidence.** A design note cites the spec that proves its invariant as the
backticked spec folder name, optionally with one failure mode after `#`: `` `0002-withdraw` `` or
`` `0002-withdraw#FM-2` `` ("Overdraw is refused as a business rule (`` `0002-withdraw#FM-2` ``)."). A note with no
spec behind it is a hypothesis; write the spec or drop the claim.

**Freshness is citation resolution** (`SKY0005`): a ctx naming a slice or type that no longer exists is stale, and
so is one citing a spec folder that does not exist or a failure mode its `spec.md` does not list. The doctor reads
the specs as AdditionalFiles (`<AdditionalFiles Include="..\..\.specs\*\spec.md" />` in the API project, beside
`**\*.ctx.md`); with none fed, every spec citation is flagged. A code citation is a single PascalCase identifier; a
spec citation starts with digits and its slug holds a letter, so the two never overlap and a backticked number
(`` `2026-31` ``, `` `2024-01-15` ``) is prose, not a citation. Not mtime: since the ctx does not duplicate code, adding
a field must not force a ctx edit.

**Kept alive by the proof loop, not a gate.** The citations are the impact index: `skies proof impact <paths>`
maps each path under `**/Modules/<M>/` to `<M>.ctx.md` and prints the specs it cites, with their failure modes, to
read before writing new ones. A note that cites nothing is invisible there, which is one more reason to cite.

## Specs and proofs

A feature is accepted by **evidence in its spec folder**, not annotations in production code. The model has two
halves: **CI owns green** (it already runs every spec's cases on every push, so a regression fails the build), and
**the receipt proves red→green once**: the cases failed before the feature and passed with it. Only red is evidence
CI cannot produce, so that is all the receipt records.

```
.specs/0012-cancel-reservation/
  spec.md          what the feature does, its failure modes (FM-1..n), what is out of scope
  e2e/             black-box tests, one or more cases per failure mode
  receipt.json     written by `skies proof record`: per failure mode, red and green, the cases, what failed on red
  red.patch        for a spec written after the code: the patch that removes the feature
  evidence/        small artifacts the cases saved (Assay verdicts, screenshots, HTTP logs), committed
    raw/           the full reports and runner output, local and gitignored
```

- **Every test lives in a spec** (`SKY0029`, `SKYFE036`, `SKYFL036`): a test written after the code guards nothing
  it names and never proved it can fail. An isolated system (a value object, a parser) gets its own spec; `e2e/`
  means "the spec's cases".
- **Failure modes come before code**, one `- FM-n <text>` line each in `spec.md` (indented lines continue it). The
  human reviews that list.
- **The test title is the only link.** A case whose name starts with `FM-n` covers that mode
  (`[Fact(DisplayName = "FM-2: double cancel refunds once")]`, `test("FM-2: …")`). No attributes or manifests. Every
  failure mode needs a case and every `FM-n` case must name a mode `spec.md` lists; otherwise the run is refused.
- **`skies proof run <spec>` is the loop while writing.** It runs the spec's E2E once on the working tree and prints
  each failure mode's pass or fail with what the failing cases reported. It writes no receipt and no committed
  evidence (its report and output stay in the gitignored `evidence/raw/`): "the cases fail now" before the code,
  "they pass now" after it.
- **`skies proof record <spec>` proves red→green.** It runs the E2E on the red revision, where every mode must fail,
  then on the working tree, where every mode must pass, and writes `receipt.json`. Red is, in order: `--red <rev>`;
  `HEAD` plus the spec's `red.patch` (`--red-patch <file>` stores one); the merge-base of `HEAD` with
  `[workspace] default_branch` from `Skies.toml`; with the current branch's upstream, when that is another branch;
  with `origin/HEAD` (else `main`, else `master`). `record` prints the choice and how far back it is (`red 5e2f092
  (merge-base with origin/main, default branch from origin/HEAD; 116 commits before HEAD)`) and warns past 50
  commits: work on a long-lived branch sets `default_branch`. A mode already passing on red is non-discriminating and
  needs a written justification under `## Non-discriminating` in `spec.md`.
- **Cases that never ran on red count as failing (`did-not-build`), whatever the runner**: no report at all (.NET E2E
  that reference a type the feature adds, or a failing `build`), or a report where no case names a failure mode and a
  case failed or the runner exited non-zero (vitest's one file-level failure for a missing import, Playwright or
  Flutter failing to load). The receipt's `red.output` keeps the lines that say why (`error CS0246: …`). Never
  applied on green. When red does not match `spec.md`, `record` prints the tail of red's output (kept as
  `evidence/raw/red.log`): the usual cause is a red revision that is not the one intended.
- **The receipt is the summary a reviewer reads.** It names the runner, red's commit (and `red.patch`), green's
  commit, and per failure mode: the red result (`fail`, `did-not-build`, `non-discriminating`), the green result,
  the cases that proved it, the start of what red's first failing case said (the assertion, not the stack; checkout
  paths become `{root}`), and for a tagged mode its criteria and verdict files. No durations, no hashes, no
  footprint: keys are sorted, so recording an unchanged spec again leaves git clean. Re-record when the failure
  modes change; nothing else asks you to.
- **Evidence is what a case chose to save.** Runners get `SKIES_EVIDENCE` (the run's evidence folder, absolute) and
  `SKIES_SPEC` (the spec folder name) in their environment, besides the `{evidence}` placeholder. What a case writes
  there is committed under `evidence/` (at most 256 KB per file; `record` refuses a larger one and says so), except
  what it writes under `raw/`, which stays local like the full reports. An Assay verdict that differs only in timings
  keeps its committed bytes. .NET tests use `SpecEvidence.Save("name.json", value)` from `Skies.Framework.Testing` (a
  no-op outside a proof run). The engine adds `/*/evidence/raw/` to `.specs/.gitignore` when missing; commit it.
- **A failure mode may name an Assay verifier**: `- FM-5 a retry with the same key credits twice
  [avp: idempotency-key-honored]` (several ids comma-separated). The case saves the verdict to
  `$SKIES_EVIDENCE/avp-FM-5.json`, and the mode passes only when its cases pass and every tagged criterion passes.
  Optional: an untagged mode is decided by its cases alone.
- **Impact comes from the ctx citations.** `skies proof impact <paths>` (without paths: the files changed since the
  base `record` would choose) maps each path under `**/Modules/<M>/` to `<M>.ctx.md` and prints the specs its design
  notes cite, all their failure modes for `` `0002-withdraw` `` and just the one for `` `0002-withdraw#FM-2` ``, plus
  every spec whose `touches:` globs in its frontmatter match a path (a screen, a copy catalog, a shared component
  outside `Modules/`). Read them before writing failure modes; the cases themselves run in CI.
- **Runners are declared in `Skies.toml`**: a shell command that runs one spec's `e2e/` and writes a JUnit or TRX
  report. The engine knows nothing about xUnit, Playwright, or Flutter:

  ```toml
  [runners.api]
  build = "dotnet build tests/App.Tests"
  command = "dotnet test tests/App.Tests --no-build --filter FullyQualifiedName~Specs.S{id}. --logger \"trx;LogFileName={report}\""
  ```

  Placeholders: `{id}`, `{spec}`, `{dir}` (the spec's `e2e/`), `{report}`, and `{evidence}`. `setup` (optional) runs
  first in each checkout (start a database, install packages in red's fresh worktree); `build` (optional) compiles
  once per checkout so `command` can skip it, and a failing build on red is `did-not-build`. The exit code decides
  nothing; the report does.

  .NET spec tests use the namespace `Specs.S<id>` so the filter selects one spec. The test project compiles them
  with `<Compile Include="..\..\.specs\*\e2e\**\*.cs" />` and nothing else, and references the doctor so `SKY0029`
  sees any test compiled from elsewhere. Declare it as the product's `tests` in `Skies.toml` so `skies doctor`
  builds it (and the backend through it).

## Testing — hosts and isolation

- **Prefer E2E through the real host.** `SkiesWebTest<TProgram>` boots the real app and hands you one hook,
  `SwapStores(IServiceCollection)`, to reconfigure services for the test. The base holds **no database opinion and
  drags no provider dependency**. Two paths, both the app's to choose:
  - *Fast and isolated*: reference `Skies.Framework.Testing.InMemory`, call
    `services.UseIsolatedInMemory<WalletsDb>()`.
  - *A real database*: reference `Skies.Framework.Testing.Postgres`: one `PostgresTestDatabase` (a single
    Testcontainers Postgres, one migrated **template** database, an isolated `CREATE DATABASE … TEMPLATE` clone per
    test, with a tiny aggressively pruned connection pool) wrapped in the app's own static accessor; register its
    connection in `SwapStores`. Keyed stores let two contexts share one database (the "written-by-one-request,
    read-by-the-next" pattern).
    **Take stores from `CreateStore(...)` and dispose the lease.** A clone is a physical copy of the migrated
    template, reclaimed only when its lease is returned. A ~700-test suite taking two or three stores each and
    returning none leaves ~1500 live databases in the shared container (measured: 14.4 GB resident, surfacing as
    Npgsql timeouts and killed test workers).
- **When a flow cannot be driven purely over HTTP** (an SMS or email code never crosses the wire), layer a capturing
  provider over the booted app through `SwapStores` / `WithWebHostBuilder`: the same seam, zero production change.
- **Every test lives in a spec; an isolated system gets its own.** A value object, a pricing calculation, a parser:
  open a spec, write down how it can fail, write one isolated case per failure mode in its `e2e/` (calling the type
  directly, no host), then write the code, and record the receipt like any other spec (a `red.patch` that breaks the
  invariant proves each case bites). Never write tests after the code to cover it. The sample's `Money` is
  `.specs/0004-money`.
- **Assertions and mocking are the app's free choice**: the kit ships and mandates none.

## The doctor — rule catalog

The doctor enforces **architecture only**; no rule demands that a test, a tag, or a manifest exists. Every rule
comes from drift observed in a real application. It catches structural drift, not logic errors: correctness is the
spec's E2E and review. Never suppress a rule: a firing rule means the shape is wrong.

Write code for the reader, not for the doctor. A comment that cites a rule id ("to satisfy SKY0018") or explains why
a line exists only to pass a check is noise; if the shape is right, the code needs no apology, and if a rule keeps
forcing awkward code, that is a finding to report against the rule.

| Rule | Enforces | Why |
|------|----------|-----|
| `SKY0001` | Slice conformance: static class; nested `Input` and `Output`; `Handle → Task<Result<T>>`; `Map`; ordered Input → Output → Handle → Map | one readable shape per feature |
| `SKY0002` | Endpoint stays thin: a route handler is an expression-bodied lambda or method group, never a statement block | business logic hides in routes |
| `SKY0004` | Every module has a `<Module>.ctx.md` with a non-empty `## Boundaries` and `## Design notes` | the why gets forgotten |
| `SKY0005` | `.ctx.md` is fresh: every backticked identifier it cites resolves in source or a reference, and every cited spec (`` `0002-withdraw#FM-2` ``) exists with that failure mode (not mtime) | ctx drifts from code and evidence |
| `SKY0006` | No `IRepository` / unit-of-work abstraction in a slice | clean-architecture bloat |
| `SKY0007` | File ≤ 500 lines (EF `Migrations/` exempt: tool-emitted, append-only) | locality for readers and agents |
| `SKY0009` | Write-ownership: a write (Add/Update/Remove/…) on another module's entity is flagged, on a `DbSet` or through `DbContext.Add(entity)`; cross-module reads, joins, and calls are free. `.Tests.cs` exempt | keeps a module carvable later |
| `SKY0012` | A `[Slice]`'s `Map` calls `.WithName("<SliceName>")` (or `nameof`): the OpenAPI `operationId` the typed client names its hook after (`use<SliceName>`). A missing `Map` is SKY0001's | backend ↔ frontend stay 1:1 |
| `SKY0013` | `[ValueObject]` has encapsulated construction and immutable properties, including init accessors, plus a Result-returning factory | validation enters through one factory; struct defaults still exist |
| `SKY0014` | `[Entity]` has no accessible constructor, setter, or init accessor | state changes through domain operations; validation behavior is tested |
| `SKY0015` | `[Module]` shape: a static class with public static `AddServices(IServiceCollection, IConfiguration)` and `Map(IEndpointRouteBuilder)` | a module's DI scatters into `Program.cs` |
| `SKY0016` | Every `[Module]`'s `AddServices` and `Map` are called in the explicit registry (`AddModules` / `MapModules`); compile-time, no reflection | a forgotten module is a silent 404 |
| `SKY0017` | `Program.cs` wires only `AddSkies`/`AddPlatform`/`AddModules` and `UseSkies`/`UsePlatform`/`MapModules`; any other service registration, pipeline step, or endpoint mapping there is flagged | the index rots into a dumping ground |
| `SKY0018` | The `code` passed to an `Error` factory / `Validation.Check` / `Validation.Add` / `FieldError` references a `const` on a `*ErrorCodes` class, never a literal | codes must be enumerable into OpenAPI and i18n |
| `SKY0019` | Every `const` on an `*ErrorCodes` registry is referenced by an `Error`/`Validation` call (the reverse of SKY0018) | dead codes ship in the enum and catalogs |
| `SKY0021` | A `DbSet<T>` whose `T` is unmarked must be `[Entity]`; a complex member of an `[Entity]` (past one nullable/collection layer) that is not `[ValueObject]`, `[Entity]`, or an enum must be `[ValueObject]`. Dead types and framework types are not flagged | an unmarked type escapes SKY0013/SKY0014 |
| `SKY0022` | Every `[Slice]` declares `.RequireAuthorization(…)` or `.AllowAnonymous()` on its `Map` chain or its module's route group. A missing `Map` is SKY0001's | a new endpoint ships open by omission |
| `SKY0023` | A `[Slice]` `Handle` that takes `ICurrentUser` and never reads it is flagged: consult the caller or remove the parameter | the ownership check was meant and dropped |
| `SKY0024` | A `*Raw` EF call (`FromSqlRaw`, `ExecuteSqlRaw`/`Async`, `SqlQueryRaw`) whose SQL interpolates or concatenates a non-literal is flagged; use `FromSql`/`ExecuteSql`/`SqlQuery`, which parameterize every hole. Constant SQL through `*Raw` stays legal | SQL injection |
| `SKY0025` | Reading `.Value`/`.Error` on a `Result<T>` held in a local or parameter with no earlier outcome check in the member (`IsSuccess`/`IsFailure`, an `is { IsSuccess: … }` pattern, a `Validation.Collect` fold) is flagged. Inline unwrap of a fresh construction (`Money.From(10m).Value`) stays legal | an exception where an `Error` should flow |
| `SKY0026` | Warning. A slice whose `Handle` mutates or updates/removes an entity with no visible token (`[Timestamp]`, `[ConcurrencyCheck]`, `RowVersion`) is flagged. Insert-only rows and entities read beside another write are excluded; fluent-only configuration is invisible, hence warning | concurrent writes silently lose data |
| `SKY0027` | Warning. `ToListAsync`/`ToList` (or array twins) ending a `DbSet`-rooted chain, directly or through a queryable local, with no `Take`/`ToPageAsync` is flagged, and so is a `Take` whose bound is a bare number instead of a named const. Parent-scoped queries are exempt (a `Where` equating or `Contains`-matching a `*Id`: the steps of one job); `OrgId`/`TenantId` equality is the tenant scope and stays flagged. Fix per the ladder above | unbounded lists degrade with data |
| `SKY0028` | Warning. The ordering chain feeding `ToPageAsync` must contain the entity's primary key (`Id`, or `{Entity}Id` on the queried entity; a foreign `*Id` does not count), else the final key is flagged. An unreadable pre-ordered local stays silent | ties repeat and drop rows across pages |
| `SKY0029` | A method with a test attribute (xUnit `[Fact]`/`[Theory]`, NUnit `[Test]`/`[TestCase]`/`[TestCaseSource]`/`[Theory]`, MSTest `[TestMethod]`/`[DataTestMethod]`, and derived ones such as `[SkippableFact]`) in a file with no `.specs` path segment is flagged; unresolved frameworks fall back to the written name (`Fact`, `*Fact`, `*Theory`, …). It asks where a test lives, never that one exists. It fires in the tests project, which references the doctor | tests written as coverage prove nothing |

**Security floor.** Beside the SKY rules the doctor raises a curated CA* set to error
(`buildTransitive/skies.globalconfig`): dropped `CancellationToken` (CA2016), insecure deserialization (CA23xx),
broken crypto, disabled certificate validation, deprecated TLS (CA53xx). Opt out per project with
`<SkiesSecurityAnalysis>false</SkiesSecurityAnalysis>`, or override one rule from your own `.globalconfig` at a
`global_level` above 50. The framework libraries hold the same floor via `build/Skies.Framework.Library.props`.

**Workspace.** `skies doctor` also checks the repository root against `Skies.toml` `[workspace] root`, natively and
without spawning a process (`--package` skips it). Entries are globs on one root entry's name; a trailing `/` is a
directory only, no slash a file only; `.git` and `Skies.toml` are implicit; `.gitignore`d entries never count. See
[MONOREPO-ARCHITECTURE.md](MONOREPO-ARCHITECTURE.md#the-root-allowlist).

| Rule | Enforces | Why |
|------|----------|-----|
| `SKYWS001` | Every root entry git would see matches `[workspace] root`: move it under a declared folder, delete it, or declare it. A manifest without `root` gets one finding with today's entries ready to paste | junk (logs, images, stray docs) piles up at the root |
| `SKYWS002` | Warning. A `root` entry that matches nothing at the root is stale: remove it | the allowlist should say what the root is |

## The self-harness — framework development only

`Skies.Framework.SelfHarness` holds Skies' own libraries to a higher bar, like the Rails repo does. It is
`IsPackable=false`, referenced with `ReferenceOutputAssembly="false"`, and never part of the CLI or
`Skies.Framework.Doctor`; it never touches an application.

| Rule | Enforces | Applies to |
|------|----------|------------|
| `SKYSELF001` | File at or under 500 lines | `Skies.Framework.*` libraries |
| `SKYSELF002` | No tracking codes or scratch markers in comments (TODO, FIXME, HACK, XXX, capital-letter/number codes) | `Skies.Framework.*` libraries |
| `CS1591` (built-in) | Every public member carries XML documentation | `Skies.Framework.*` libraries |

Settings live once in `build/Skies.Framework.Library.props`.

## Scope and non-goals

**In:** the standard project shape, the doctor, the ctx discipline, the thin wire (`Result<T>`, `[Slice]`,
`[ValueObject]`, `[Entity]`), the scaffolders, and spec receipts.

**Out, by decision:** source generation of behavior (a mini-compiler); vendor adapters in core (they follow the
conventions in the app or a separate repo); UI source generation, real-time on by default, multi-app sprawl (the
frontend is scaffolded once and owned; hubs are opt-in); a runtime framework to inherit from; gates (no installed
hook, no check that audits the agent, no policing of CI, suppressions, or package versions).

Hidden source generation, a DSL, a base class, magic discovery, or capability instead of convention and
enforcement is a scope violation. Reject it in line.
