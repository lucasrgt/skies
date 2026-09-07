# Skies (.NET) — Conventions & Constitution

Skies is the **opinionated .NET convention bundle**: a standardized vertical-slice
architecture + a build-time harness (the doctor) + an ai-context discipline, so an LLM has
less to decide and what it writes is enforced. It is the *Rails mindset in .NET* — the
mentality (convention over configuration, quality control, semantic density), **not** the
mechanism (no runtime metaprogramming, no language).

The goal is **not "less code"**. The goal is **semantic density**: each unit carries more
meaning per token for the AI. Token savings follow from that — they are not the target.

---

## The gift, and the boundary

Skies's gift is **the scaffolding and the harness** — the opinionated project shape, the generators
(`skies new` / `g`), and the doctor that enforces the conventions. That is the whole product. It is
**not** a platform that absorbs every integration, vendor, or business rule.

This is the Rails posture. Rails ships convention + the framework; the rest is **gems** the app
chooses and **business logic** the app writes. Skies is the same — it ships the skeleton and the
enforcement; each project brings its own libraries (a hashing lib, a payment SDK, a maps client) and
its own domain rules, in plain idiomatic C#. The framework never grows a feature to meet one app's
specific need.

The boundary is deliberate — the lesson of two failures we will not repeat:

- **The predecessor language** grew into a gargantuan apparatus — per-vendor adapters, a runtime, a
  compiler — and shipped to nobody.
- **aerocoding** became a "unification engine," building artifacts for features that did not exist.

Both died of *trying to absorb the world*. So the test for any proposed framework feature is one
question: **is it generic across projects and proven by real use?** If it smells like the framework
conforming to one app — a vendor adapter, a domain rule, a bespoke capability — it is out; it lives in
the app. The framework earns new surface from evidence, never from the ambition to be comprehensive.

---

## The two laws (read before adding anything)

1. **Stranger-maintainable.** The output is always plain, idiomatic C# that a .NET dev who
   has never heard of Skies can read and maintain. (The predecessor language died failing this:
   its real code was generated Go.)
2. **Doctor-removable.** `dotnet remove` the analyzer and the project still **compiles and
   runs** — you only lose enforcement. The harness is wire, not apparatus.

Any feature that fails both laws — hidden source-gen of behavior, a DSL, a runtime you
inherit from, magic discovery — is **out**, by construction.

---

## The slice convention

One feature = **one file** (maximal locality: the agent reads the whole feature in one
read). The canonical shape (enforced by `SKY0001`):

```csharp
[Slice]                                  // pure marker; module is derived from the namespace
public static class Deposit
{
    public record Input(/* ... */);                            // contract in — visible
    public record Output(/* ... */);                           // contract out — visible

    public static async Task<Result<Output>> Handle(           // the essence — visible
        Input input, AppDb db, CancellationToken ct)
    {
        // DbContext direct. No repository, no unit-of-work, no mapper profile.
        // Behavior always lives here, never hidden.
    }

    public static void Map(IEndpointRouteBuilder app) =>       // transport — one thin line
        app.MapPost("/deposit", async (Input input, AppDb db, CancellationToken ct) =>
                (await Handle(input, db, ct)).ToHttp())
            .WithName(nameof(Deposit));                        // the operationId the typed client hooks from (SKY0012)
}
```

- **DbContext direct** in the handler. The repository/UoW layer is the clean-architecture
  bloat we cut; the doctor forbids reintroducing it (`SKY0006`).
- **Handlers are HTTP-agnostic.** They return `Result<T>`; the API boundary maps it to a
  status code (`ResultHttpExtensions.ToHttp`). This keeps them unit-testable without a host.
- **Errors carry a code, not just copy.** An `Error` is `(Kind, Code, Message[, Fields])`. `Kind` is the closed
  category that maps to the HTTP status; **`Code`** is a stable, language-neutral, namespaced key — `<module>.<reason>`
  (e.g. `wallets.insufficient_funds`), value objects use `<vo>.<reason>`, field errors `<field>.<reason>` — that the
  **frontend localizes from**; `Message` is a developer hint (English, like a log line), never the copy a user reads.
  The factories require it — `Error.NotFound(code, message)`, `Validation.Check(ok, field, code, message)` — while
  `Collect` inherits the value object's own code.
- **A code is a registry constant, not a literal.** Each module owns a `<Module>ErrorCodes` static class of
  `const string` codes — the readable catalog of what can go wrong there — and every `Error`/`Check`/`FieldError`
  references one (`WalletsErrorCodes.NotFound`), never a bare string. The doctor (`SKY0018`) enforces it, which keeps
  the set **discoverable**: `AddSkiesOpenApi` reflects over the registries and enumerates them into the
  `ErrorBody.code` schema, and `ToHttp` advertises the `ErrorBody` envelope on every endpoint — so the generated
  client is typed on the closed set of codes and the frontend's i18n can be checked for an exhaustive translation of
  each (localization stays the frontend's job — copy has one owner — the backend ships the keys). Platform-tier
  failures follow the same discipline: codes for cross-cutting concerns live on a `PlatformErrorCodes` registry —
  the framework ships its own (`platform.rate_limited`, rendered by `RejectAsSkiesError()` when the app wires
  ASP.NET's rate limiter), and an app adds platform codes of its own the same way — so even a 429 reaches the
  client as a localizable `ErrorBody`, never a bare status.
- **Rich types carry the semantics.** Prefer `Money`, `Cpf`, `Email` over `decimal`,
  `string`, and let entities own their invariants (see **The domain** below). The type *is*
  the rule — the AI understands it without reading a validator.
- **A module owns both halves of its wiring** — `AddServices(IServiceCollection, IConfiguration)` (its own DI)
  and `Map(IEndpointRouteBuilder)` (its routes) — and is marked `[Module]`. An explicit registry (`Modules.cs`,
  with `AddModules` / `MapModules`) lists every module on both sides. No reflection, no discovery: the registry
  is plain code, and the doctor (`SKY0015` / `SKY0016`) checks the shape and that every `[Module]` is registered.
- **The composition root is three named layers, not a dumping ground.** `Program.cs` stays a thin index —
  `AddSkies()` + `AddPlatform(config)` + `AddModules(config)`, then the matching `UseSkies()` /
  `UsePlatform()` / `MapModules()`. **`AddSkies`** is the framework's universal conventions (OpenAPI,
  enum-as-name JSON), shipped by Skies.Framework.AspNetCore. **`AddPlatform` / `UsePlatform`** is the app's own
  cross-cutting infrastructure — its `DbContext`, auth, CORS, the framework ports it shares: app-owned (the
  framework can't know your store or your vendors), a *conventional name* so every Skies backend reads the
  same, and simply absent when there is nothing cross-cutting to share. It splits **by concern** — one
  `Platform/<Concern>.cs` per concern (recommended vocabulary: Persistence, Security, Observability, Web),
  each a partial of one `Platform` class, composed explicitly (no discovery); a single-concern app is just one
  `Platform.cs`. It grows by adding a concern file, never by fattening `Program.cs`. **`AddModules`** is the
  registry above. A vendor or domain service belongs in the module that owns it (its `AddServices`), never the
  platform. The doctor (`SKY0017`) keeps the index pure: any service registration, pipeline step, or endpoint
  mapping that leaks into `Program.cs` is a build error, redirected to the platform or a module — so the index
  can't rot back into a dumping ground.
- **Authorization is a decision, never an omission.** Every slice's endpoint carries an explicit posture —
  `.RequireAuthorization(…)` or `.AllowAnonymous()` — on its own `Map` chain or on the module's route group
  (`app.MapGroup("/wallets").RequireAuthorization()`); an endpoint with neither is a build error (`SKY0022`).
  Inside the handler, the caller arrives as an injected `ICurrentUser` (claims-based, from `Skies.Framework.Auth`) and
  the slice does its own ownership/role/org check — a `Handle` that injects `ICurrentUser` and never reads it
  is flagged (`SKY0023`): the signature would claim a check the body doesn't make.
- **Co-located `<Module>.ctx.md`** carries the business "why" — the rules that are not in the
  control flow. One per module (not per slice). Shape + rationale in
  [The ctx.md schema](#the-ctxmd-schema); presence + spine are gated by `SKY0004`.
- **LAW — validation is always inline at the top of the Handle; never extracted to a method.**
  Build the value objects, accumulate with `Validation` (`Check` for an inline condition,
  `Collect` for a value object's verdict, and the shorthands for the recurring shapes —
  `Require(guid, field, code)`, `NotBlank(text, field, code)`, `InRange(value, min, max, field, code)`),
  then `if (validation.Failed) return validation.ToError();`. There is no per-slice judgment and no "extract when it grows": if the
  inline validation ever feels too large, the fix is to push rules into value objects (where the
  rule belongs), never to extract a `Validate` method. Validation's complexity lives in the
  types, so the inline part stays a short list — there is nothing big to extract.

### Pagination — the canonical page

A paginated list slice returns the framework's one page shape, the collection analogue of `Result<T>`:
without it every slice invents its own output, the generated client gets an ad-hoc type per list, and
nothing downstream (the typed client, the frontend spine's pager hooks) can recognize "this is a page"
and compose. The pieces:

- **`Page<T>(Items, TotalCount, PageNumber, PageSize)`** lives in `Skies.Framework.Abstractions` beside `Result<T>`.
  The numbers are the **effective** values after server-side clamping, echoed — the contract never reports
  a page that was not actually served. `AddSkiesOpenApi` pins the schema (four members required, plainly
  numeric, a collision-free slice-qualified id), so the spine's structural `Page<T>` match holds end-to-end.
  Since 0.4.0 the plain-numeric pin covers the **whole document**: `NumberHandling`'s read-from-string
  tolerance is a runtime affordance the serializer never writes, so every numeric schema — body property,
  query parameter, inline sub-schema — declares the plain number the wire actually speaks (nullability
  survives). No more `number | string` unions pushing `Number(x) || 0` coercions into ViewModels.
- **`ToPageAsync(pageNumber, pageSize, maxPageSize = 100, ct)`** lives in the **`Skies.Framework.EntityFrameworkCore`
  satellite** (the only runtime package that references EF Core — an app opts in à la carte; it is not in
  the `Skies` meta-package). The receiver is **`IOrderedQueryable<T>`, not `IQueryable<T>`**: paginating
  without an `OrderBy` does not compile — an unordered Skip/Take has no stable meaning in SQL — and that
  enforcement is the type system's, so it survives even with the doctor removed. Count and page run over
  the **same queryable**, so a count taken before a tenant filter (leaking other orgs' existence into the
  total) cannot be written. Filter first — `AcrossOrgs()`, `Where(...)` — then order, then page.
- **Order by a unique key.** `OrderBy(x => x.Name).ThenBy(x => x.Id)` — equal sort values with no
  tiebreaker make page boundaries non-deterministic (rows repeat and vanish between pages). The doctor's
  `SKY0028` (warning) reads the ordering chain feeding `ToPageAsync` and flags the final key when no key
  in the chain is the entity's primary key — a member named `Id`, or the EF-conventional `{Entity}Id`
  declared on the queried entity itself. A *foreign* `*Id` (`CustomerId` on a Wallet) is many-rows-shared
  and earns nothing. An ordering the analyzer cannot read (a pre-ordered local crossing the statement)
  stays silent — the warn speaks only when it can see the keys.
- **The idiom: page the ordered entity, project the page in memory.** `.Select(...)` erases the
  `IOrderedQueryable<T>` the extension requires — by design. Ordering *after* a `.Select` compiles but is
  not the way out: EF does not translate an `OrderBy` over a positional-record projection (it inlines the
  constructor into the OrderBy and gives up at runtime — pauta hit this). Page first, then project the
  (small) page with `Page<T>.Select`; aggregates join the page's ids afterwards:

  ```csharp
  var wallets = await db.Wallets.OrderBy(w => w.Id).ToPageAsync(input.Page, input.PageSize, MaxPageSize, ct);
  return new Output(wallets.Select(w => new WalletView(w.Id, w.Balance.Amount)));
  ```

- **The slice's `Input` stays flat** (`int Page = 1, int PageSize = 20`) — the visible contract; there is
  no `PageRequest` envelope on the wire, and `MaxPageSize` is the server's policy (a slice-local const),
  never a payload. Aggregates beside the list travel by **composition** —
  `record Output(Page<ReviewView> Reviews, double AverageRating)` — never record inheritance.
- **The count is explicit in the contract.** `TotalCount` exists ⇒ a `COUNT(*)` ran; that is the deal a
  numbered pager needs ("1–20 of 87") and it is deliberately visible, not hidden behind a flag. When a
  pilot one day needs a high-volume infinite feed, cursor pagination arrives as a **second** primitive
  (`CursorPage<T>`), never unified with the offset shape into one premature abstraction.
- The canonical slice is the sample's `ListWallets`; the doctor's `SKY0027` (warning) flags a
  `DbSet`-rooted query materialized with no `Take`/`ToPageAsync` — the list that ships fine at ten
  development rows and degrades as a tenant's data grows.
- **When `SKY0027` fires, the remediation ladder** (one answer per shape — never a third invention):
  1. **The set has a domain bound you can name** → `.Take(N)` where `N` is that bound as a named
     const (`MaxQueue`), the comment saying *why* the set is small. A generous cap you cannot
     justify is not a fix — it is silent truncation at overflow with no UI affordance, a lie with
     a green build. If the only honest comment is "probably small", it is not a `Take` site.
  2. **The set accretes with usage** (an inbox, an agenda, a history, "my reservations") →
     migrate to `ToPageAsync`/`Page<T>` **even before any paging UI exists**. Page 1 at a generous
     size is the same response the client gets today with the contract made honest; the spine's
     pager hooks (`usePager`, `useAccumulatedPages`) are already waiting when the UI catches up.
  3. **A write path over a set that is not aggregate-scoped** (purge expired rows, bulk
     re-status) → the materialization is still the defect, but the remedy is set-based
     `ExecuteUpdateAsync`/`ExecuteDeleteAsync` (or a batched job) — never a synthetic `Take`,
     which would simply *not perform* part of the write.
  4. **None of the above yet** → leave the warning standing. Warning tier *is* the adoption
     ledger: an open `SKY0027` is a decision still pending, and a release may ship with pending
     decisions. Suppressing the rule is never the move.
- **What the parent-scope exemption deliberately does not see**: `Where(m => m.ChatId == id)` is
  exempt because most child sets are bounded by their parent's cardinality (the steps of one job,
  the sessions of one user) — but an **accreting child** (one chat's messages, one aggregate's
  audit trail) grows without bound and passes silently. The exemption trades that recall for
  precision (a false positive teaches suppression; a false negative merely doesn't teach). Paging
  an accreting child is a design call the doctor cannot make for you — make it at design time,
  rung 2 of the ladder.

### The contract never mints a client route

When a slice's output drives a navigation (a pending-task card, a CTA, a "go fix this" affordance),
the backend decides **which action exists** — never **where the client navigates**. The payload
carries a **closed enum** (`PendingKind`-style, a plain C# enum in the `Output`); the client owns the
kind→route map over the *generated* enum (see FRONTEND-CONVENTIONS.md — server-driven actions). A
route string minted server-side (`CtaTarget = "/host/properties/new"`) is the documented
anti-pattern: it is runtime data, invisible to OpenAPI, to `tsc`, and to the router's typed routes —
**no gate closes that contract**, and a pilot shipped exactly this bug to prod (two server-minted
routes didn't exist in the app → 404 on tap). With the enum, the same drift is a compile error on
both sides: a new kind breaks the client's exhaustive `Record` until it is mapped, and the mapped
value is a typed route.

The same boundary line generalizes: the server speaks **domain vocabulary** (kinds, statuses, codes —
closed enums and registry constants); the client owns the **presentation mapping** (route, copy,
icon). It is the error-code discipline (`SKY0018`/`SKY0019`: stable code on the wire, copy in i18n)
applied to navigation.

*Analyzer decision (2026-06): evaluated and **not shipped**.* The heuristic — flag a `"/"`-prefixed
string literal in a slice-payload property named `*Target`/`*Route`/`*Href`/`*Path` — has plausible
false positives that are themselves legitimate contract data (API paths, storage object paths,
webhook URLs), and the consumption side is already gated mechanically: the client cannot navigate to
an arbitrary server string without a cast, and `SKYFE030` makes that cast an error. A false positive
teaches suppression; the convention + the front-side gate close the loop. Revisit only if a pilot
ships another server-minted route *after* this convention landed.

---

## The domain — entities & value objects

A slice is the *operation*; the **entity** and its **value objects** are the *domain* it operates on. Skies
keeps the tactical-DDD half that earns its keep — rich, self-validating types — and leaves the apparatus
(repositories, a base class you inherit, an event bus for internal decoupling) out, because each fails one of
the two laws. The result is plain C# that happens to be hard to misuse.

- **Value objects (`[ValueObject]`) are always-valid by construction.** An identity-less domain value
  (`Money`, `Cpf`, `Email`) is immutable, has no public constructor, and is built only through a static smart
  constructor returning `Result<T>` — the `Money.From` shape. Because an invalid instance can never come to
  exist, there is no "validate afterwards" step to forget: any `Money` in the system is already valid. `SKY0013`
  enforces the shape. Value objects live in `BuildingBlocks/` when generic, inside the module when specific.

- **Entities (`[Entity]`) encapsulate state and guard invariants.** An entity has identity and a lifecycle. It
  exposes no public constructor (it is born through a static factory like `Wallet.Open`, and EF rehydrates it
  through a private parameterless one), no public setter (state changes only through intention-revealing
  methods like `Deposit` / `Withdraw`), and a single private invariant funnel — `EnsureValid` returning
  `Result<T>` — that every create and mutate path returns through. So the entity can never be observed or
  persisted broken, and the invariant cannot be bypassed by a slice that forgets to check. `SKY0014` enforces
  the shape. The required private parameterless constructor is exactly the one EF Core materialises through —
  the convention and the ORM ask for the same thing, so encapsulation costs nothing at the storage boundary.
  An entity that a persisted write touches also declares its **concurrency posture**: a `[Timestamp]
  public byte[]? RowVersion { get; private set; }` (or `[ConcurrencyCheck]` on a domain field), so two
  concurrent requests can't silently last-write-win each other —
  `SKY0026` (warning-tier during `skies doctor`, blocking in `skies gate`) watches for the missing token. It reports
  tracked updates/deletes, not insert-only rows or entities merely read beside another write: a concurrency
  token cannot improve either of those cases.

- **Scalar value objects are transparent on the wire.** A scalar `[ValueObject]` that crosses the API
  boundary subclasses `ScalarJsonConverter<TVo, TPrimitive>` (next to the type, pointed at by
  `[JsonConverter]`): it serializes as the primitive it wraps (`Money` as its number, `Slug` as its string),
  invalid wire input fails through the smart constructor as a 400, and `AddSkiesOpenApi` mirrors the
  primitive in the contract schema automatically — so the generated client types it as the primitive, never
  an empty object. The richness is a backend guarantee, not a contract change. (Earned in the hostpoint
  pilot, where every wire-crossing scalar VO needed this by hand.)

- **Where behaviour lives — the split with the slice.** The slice owns *orchestration* and *input validation*
  (inline at the top of `Handle`); the entity and its value objects own *invariants* and *state transitions*.
  A change that cannot fail stays a `void` method (`Wallet.Deposit` — `Money` already guarantees a non-negative
  amount); a change that can violate a rule returns `Result<T>` and funnels through `EnsureValid`
  (`Wallet.Withdraw` refusing an overdraw). This is the validation LAW carried from value objects to entities —
  *push the rule into the type where it belongs* — and none of it is hidden: the entity sits at the module
  root, one read from the slice, in plain C#.

The markers are **pure markers** (like `[Slice]`): no base class, nothing to inherit, no EF semantics. Delete
the doctor and they become inert decoration — the domain still compiles and runs (Law 2).

- **The mark is not optional where the type is persisted or owned.** `SKY0013`/`SKY0014` grade a type that is
  *already* marked — so leaving the mark off used to be a silent way to skip enforcement entirely (the pauta
  port shipped an anemic `User` table this way). `SKY0021` closes that on-ramp from below: a type that is a
  `DbSet<T>` (a table) must be `[Entity]`, and a complex member of an `[Entity]` must be `[ValueObject]` —
  the two places where "what this is" is structurally certain. It does not guess beyond those two signals: a
  DTO, an options bag, or a dead unused record is not forced to wear a mark. See
  [the decision](decisions/skies-framework-unmarked-domain-type.md).

---

## Project layout

```
src/<App>.Api/
  Program.cs                       # composition root, a thin index: AddSkies + AddPlatform + AddModules (+ the matching Use*/Map*)
  GlobalUsings.cs
  AppDb.cs                         # one DbContext for every module — the modular monolith's store
  Platform.cs                      # the app's cross-cutting infra: AddPlatform / UsePlatform — app-owned, optional
  Platform/<Concern>.cs            #   ...or a folder, one concern per file (Persistence/Security/Observability/Web): partials of Platform
  Modules/Modules.cs               # the module registry: AddModules + MapModules wire each [Module] (explicit)
  Modules/<Module>/                # a logical bounded context (owns + writes only its own entities)
    <Module>Module.cs             #   the module's wiring root ([Module]): AddServices (its DI) + Map (its routes)
    <Module>.ctx.md               #   the module's "why" — Boundaries + Design notes (see schema)
    <Entity>.cs                   #   entities live at the module root — domain, not operations
    Slices/<Name>.cs              #   one slice = one operation (Input/Output/Handle/Map)
    Slices/<Name>.Tests.cs        #   tests co-located with the slice they exercise
  BuildingBlocks/                  # shared value objects (Money, Cpf) — generic, owned by no module
  Journeys/<Name>Journey.Tests.cs  # happy + sad E2E journeys required for every write slice
tests/<App>.Tests/                 # thin runner: globs the co-located *.Tests.cs, provides TestApp
```

- **Slices live in `Slices/`; the domain (entities, DbContext) lives at the module root.** The split
  keeps a module with ten operations from swallowing what is domain. The folder is `Slices/`, not
  `Features/` — one term, matching `[Slice]`, `skies g slice`, and the rules.
- **Value objects go in `BuildingBlocks/`** when generic (a leaf type couples nothing), or inside the
  module when module-specific.
- **The namespace is `<App>.Api.Modules.<Module>` — the `Slices/` subfolder is not in it.** Folders
  organize files; the namespace is the module.
- **Modular monolith: one `AppDb`, modules are bounded contexts by convention.** Every module shares one
  DbContext, so a read can join across modules in-process — a dashboard (the host home) is one query, not a
  composition. What keeps a module liftable later is cheap discipline, not isolation: it **writes only its
  own entities** (SKY0009) and references other modules **by id, never an EF foreign key**. Reads / joins /
  in-process calls across modules are free; a cross-module *effect* goes through the owner's service or a
  job, and domain events + an outbox are reserved for genuinely-async external integrations (a payment
  webhook), not internal decoupling. Extracting a module later re-platforms only its cross-module reads — a
  move, not a rewrite. (See `skies-framework-modular-monolith` in the decisions archive.)
- The scaffold (`skies new`) and generators (`skies g module` / `g slice`) produce exactly this shape.

---

## Real-time — hubs (opt-in)

Real-time is **opt-in**, the way Rails 8 dropped the default `channels/` folder: a fresh app has no hub, so
it carries no transport it doesn't use. When a feature needs server-pushed liveness — live messages, typing,
presence — `skies g hub <Module> <Name>` scaffolds a SignalR hub under `Modules/<Module>/Realtime/<Name>Hub.cs`.

- **A hub is wire, not logic — the same law as an endpoint.** A hub method persists nothing itself: it calls
  the matching slice (the one source of the write + its rules) and then fans the result out to the room.
  Ephemeral signals (typing, presence) ride the hub and never touch the database. SignalR is first-party
  ASP.NET Core — wire, not a vendor; the harness stays doctor-removable.
- **The caller comes from `Context.User` via `ClaimsCurrentUser`**, not the request-scoped `ICurrentUser` — a
  hub method runs outside the HTTP pipeline, where `IHttpContextAccessor.HttpContext` is not reliably set. The
  token rides the query string for hub paths (a WebSocket can't send an `Authorization` header); the generator
  prints the `Program.cs` wiring.
- **Webhook ≠ hub.** An inbound provider callback (a payment webhook) is a normal slice with a route, not a
  hub. A hub pushes to *your* connected users; it never receives third-party callbacks.
- **Ephemeral state is not an entity.** Presence and typing are in-memory (a singleton registry) on a single
  instance; scaling to several instances swaps in a backplane + shared store (Redis, TTL heartbeat) — one line
  at the composition root, the only place that changes. No database write per heartbeat.

---

## The ctx.md schema

Every module carries one `<Module>.ctx.md` — the home for the business *why* the code cannot show. It
is **not** a mirror of the code: anything recoverable from the types, the tests, or the routes is
duplication, and duplication rots. (corbanx's 16-section feature doc was the reference; we graded its
sections against that one test and kept about a third.)

**Spine — required, gated by `SKY0004`:**

- `# <module>` + a 1–3 line purpose paragraph.
- `## Boundaries` — Inside / Outside (and non-goals). Where the module's seams are, and where *not* to
  add code. The highest-value section: it stops scope leak.
- `## Design notes` — the invariants and their rationale: the non-obvious rules and *why* they hold (the
  "gems"). Performance, security, and cross-module side-effects fold in here when they carry a why.

**Optional — add when earned:**

- `## Wiring` — integration dependencies not obvious from the imports (e.g. "org provides tenancy").
- `## Not yet ported` — deliberate absences, so a reader knows they are intentional, not forgotten.

**Excluded by law** (recoverable → would duplicate and drift): data models (the entities *are* the
model), DTO/contracts (the `Input`/`Output` records *are* the contract), route tables (the `Map` *is*
the route), error tables (`ToHttp` owns the mapping), test matrices (the `*.Tests.cs` *are* the matrix),
request/response examples, code-pointer file lists, and change logs (procedence is narrative, not
normative — decisions go to an ADR + git, never the ctx). **No Mermaid / flow diagrams** either: a
non-linear flow is explained in prose in the Design notes.

`SKY0005` keeps it honest by **freshness as citation-resolution**, not mtime: a ctx that names a slice or
type which no longer exists is stale. An mtime rule would be wrong here — because the ctx does not
duplicate the code, adding a field must *not* force a ctx edit. A citation resolves against this module's
source, a referenced assembly, **or a co-located `*.Tests.cs`** the project feeds the doctor as an
`AdditionalFile` — so a module ctx may name the journey that covers it (a `[Journey]` class is
`<Compile Remove>`'d from the api compilation, hence invisible as a *symbol* but readable as text), without
the false "no longer exists" it would otherwise raise.

---

## Testing — co-located, categorized, host-provided

Tests live **next to the slice** (`Deposit.Tests.cs` beside `Deposit.cs`): the app assembly
excludes them (`<Compile Remove>`) so no test dependency ships; a thin per-app test project
compiles them via glob. No mirror-the-architecture test tree.

- **Categories are a closed vocabulary** (`Skies.Framework.Testing`): `[Unit]` (fast, no infrastructure),
  `[Integration]` (real infrastructure — database, HTTP), `[E2E]` (cross-module journey). Each
  maps to the xUnit trait `Category=<kind>`, so `dotnet test --filter Category=Unit` and the
  doctor read the same single signal — the attribute on the test.
- **The integration host is `SkiesWebTest<TProgram>`.** It boots the real app and hands you one
  hook, `SwapStores(IServiceCollection)`, to reconfigure services for the test. The base holds
  **no database opinion and drags no provider dependency**. Two paths, both the app's to choose:
  - *Fast and isolated* — reference `Skies.Framework.Testing.InMemory`, call
    `services.UseIsolatedInMemory<WalletsDb>()`.
  - *A real database* — reference `Skies.Framework.Testing.Postgres`: one `PostgresTestDatabase` (a single
    Testcontainers Postgres, one migrated **template** database, an isolated `CREATE DATABASE …
    TEMPLATE` clone per test, with a tiny aggressively pruned connection pool) wrapped in the app's own static accessor; register its
    connection in `SwapStores`. Keyed stores let two contexts share one database (the
    "written-by-one-request, read-by-the-next" pattern). The in-memory package never enters the
    graph. (Graduated from the hostpoint pilot's `TestDatabase`.)
    **Take stores from `CreateStore(...)` and dispose the lease.** A clone is a physical copy of the
    migrated template, so a database is only reclaimed when its lease is returned — keyed stores when
    the last holder returns theirs. A ~700-test suite asking for two or three stores each and returning
    none leaves ~1500 live databases in the shared container; the WSL2/Docker VM grows with them and the
    host starts killing processes (measured: 14.4 GB resident, the symptoms surfacing as Npgsql timeouts
    and killed test workers rather than as anything database-shaped). `CreateDatabase(...)` returns a bare
    connection string with no lifetime and cannot reclaim anything — it remains only for callers written
    before leases existed, and the class warns on stderr once a run accumulates enough un-returned clones.
- **Assertions and mocking are the app's free choice** — the kit ships and mandates none
  (FluentAssertions, Shouldly, NSubstitute, xUnit-native all work). The one deliberate coupling is
  the **runner: xUnit** (the categories are xUnit traits).
- `SKY0003` makes every slice carry a co-located test; the build fails without one.
- **Acceptance verification is universal.** Every `[Slice]` declares at least one criterion in its module's
  `<Module>.spec.toml` (`SKY0031`) and has a subject-bound `[AVP(typeof(Slice), "criterion-id")]` proof (`SKY0030`).
  The proof runs in the official gate; a disabled test is a failure (`SKY0032`). There is no lighter slice class.
- **Every write proves failure, not just success.** A write slice is covered end-to-end on **both** paths: a
  happy journey and at least one sad journey, each declared with `[Journey(typeof(Slice),
  JourneyPath.Happy|Sad)]` on the `[E2E]` test. `SKY0008` derives the debt from ordinary code: persistence calls
  and write endpoint verbs are writes; set-based EF updates/deletes and raw/interpolated SQL execution count even
  without `SaveChanges`. A visible `MapGet` with no write signal is a read; an unfamiliar or ambiguous shape
  receives the stronger write bar. The implementing agent never supplies the classification.
  The sad journey asserts the failure status **and** that no state changed.
- **`[Journey]` is not a category — it is the proof of a write slice, both ways.** `[Unit]` / `[Integration]` /
  `[E2E]` are the categories (pick one); `[Journey]` binds an `[E2E]` test to the write it covers. `SKY0008`
  enforces write ⇒ happy + sad journeys and `SKY0010` enforces journey ⇒ write. A voluntary read traversal is a
  plain `[E2E]` flow. Naming follows: **`*Journey.Tests.cs`** = write-slice proofs,
  **`*Flow.Tests.cs`** = plain `[E2E]` flows.
- **Verification has no application-controlled mode.** The proof depth above is an unconditional invariant;
  `Skies.toml` only declares topology and rejects non-topology sections. No attribute, CLI flag, manifest
  key, skip, or exception can lower the bar. The former classification attributes are absent from the package,
  so using one is a compiler error rather than tolerated decoration.
- **A generator that scaffolds a write ships its journeys too.** `skies g slice`, `skies g auth`
  (`Login`/`Refresh`/`Register`) and every augment (`g auth:otp` → `VerifyPhone`) emit matching
  `Journeys/*.Tests.cs`. The generic write scaffold is deliberately red until both journeys prove their terminal
  state. When a journey cannot be driven purely over HTTP (an SMS or email code never crosses the wire), the
  generated journey layers a **capturing provider** over the booted app through the `SwapStores` /
  `WithWebHostBuilder` hook — the same test seam, with zero production change.

---

## The doctor — rule catalog

Every rule is born from observed pain (the corbanx pilot + the Rails-repo discipline),
never speculation. Keep it minimal; add only on real drift.

| Rule | Enforces | Status | Origin |
|------|----------|--------|--------|
| `SKY0001` | Slice conformance (static class; nested `Input` and `Output`; `Handle → Task<Result<T>>`; `Map`; ordered Input → Output → Handle → Map) | **shipped** | corbanx: "deviated from architecture" |
| `SKY0002` | Endpoint stays thin (a route handler is an expression-bodied lambda or method group, never a statement block) | **shipped** | corbanx: "business logic in routes" |
| `SKY0003` | Every slice has a co-located `<Slice>.Tests.cs` | **shipped** | corbanx: "didn't write tests" |
| `SKY0004` | Every module has a `<Module>.ctx.md` with the spine (`## Boundaries` + `## Design notes`, non-empty) | **shipped** | corbanx: "forgot to write ai.context" |
| `SKY0005` | `.ctx.md` is fresh — a backticked identifier it cites resolves in source, a reference, or a co-located `*.Tests.cs` (not mtime) | **shipped** | corbanx: "ai.context drifted" |
| `SKY0006` | No `IRepository` / unit-of-work abstraction in a slice | **shipped** | clean-arch bloat cut |
| `SKY0007` | File ≤ 500 LOC (EF `Migrations/` exempt — tool-emitted, append-only) | **shipped** | Rails-repo discipline |
| `SKY0008` | **Every shape-derived write slice has happy and sad journeys**. Persistence/write endpoint signals classify it; a proven `MapGet` is read; ambiguity fails closed as write | **shipped** | self-classification let ordinary production writes evade E2E |
| `SKY0009` | **Write-ownership**: a module writes only its own entities — a write (Add/Update/Remove/…) on another module's entity is flagged, on a `DbSet` *or* through the untyped `DbContext.Add(entity)` form; reads/joins/calls across modules are free. `.Tests.cs` exempt | **shipped** | modular monolith — write-ownership keeps a context carvable later |
| `SKY0010` | A `[Journey]` covers a shape-derived write slice — the inverse of `SKY0008`. Use plain `[E2E]` for a voluntary read flow | **shipped** | journey metadata must map to an enforced obligation |
| `SKY0011` | **Tests live in `src/`**: a test method (`[Fact]`/`[Theory]`/`[Unit]`/`[Integration]`/`[E2E]`/`[Journey]`) authored outside a `src/` directory is flagged. The `tests/<App>.Tests` project is infrastructure only (WebApplicationFactory, DB harness, shared bootstrap); unit tests sit next to their slice, journeys under `src/.../Journeys` | **shipped** | keeps tests co-located + doctor-visible; the runner project stays pure infra |
| `SKY0012` | **Endpoint named after the slice**: a `[Slice]`'s `Map` must call `.WithName("<SliceName>")` (or `nameof`). That name is the OpenAPI `operationId` the typed client generates its hook from (`use<SliceName>`), keeping backend↔frontend 1:1. A missing `Map` is SKY0001's concern, not this rule's | **shipped** | the back→front naming seam — a forgotten name drifts the generated client |
| `SKY0013` | **Value object always-valid**: a `[ValueObject]` is immutable, exposes no public constructor and no public setter, and is built only through a static smart constructor returning `Result<T>` (the `Money.From` shape) — so an invalid instance can never exist | **shipped** | anemic domain — a value must be unconstructable when invalid, not validated after the fact |
| `SKY0014` | **Entity encapsulation + invariant funnel**: an `[Entity]` exposes no public constructor (born via a factory, rehydrated by EF via a private one) and no public setter, and declares a private `EnsureValid()` (or `Validate()`) returning `Result<T>` that every create/mutate path returns through | **shipped** | anemic domain — invariants must live on the entity, unbypassable (the sample's own `Wallet` was a setter bag) |
| `SKY0015` | **Module shape**: a `[Module]` is a static class declaring a public static `AddServices(IServiceCollection, IConfiguration)` (its own DI) and a public static `Map(IEndpointRouteBuilder)` (its routes) — it owns both halves of its wiring | **shipped** | the composition root drifts into a dumping ground; a module's DI scatters across `Program.cs` |
| `SKY0016` | **Module registered**: every `[Module]`'s `AddServices` and `Map` are actually called in the explicit registry (`AddModules` / `MapModules`) — a compile-time reachability check, no reflection | **shipped** | generating a module and forgetting to wire it — a silent 404 instead of a build error |
| `SKY0017` | **Composition root is an index**: the top-level statements (`Program.cs`) wire only `AddSkies` / `AddPlatform` / `AddModules` and the matching `UseSkies` / `UsePlatform` / `MapModules`; any other service registration (`IServiceCollection`), pipeline step, or endpoint mapping (`Use*`/`Map*`) there is flagged and redirected to the platform or a module | **shipped** | the index rots into a dumping ground — infra creeps back into `Program.cs`, drifts, and bit-rots there unwatched |
| `SKY0018` | **Error code is a registry constant**: the `code` passed to an `Error` factory / `Validation.Check` / `Validation.Add` / `FieldError` must reference a `const` on a class named `*ErrorCodes` (e.g. `WalletsErrorCodes.NotFound`), never an inline literal | **shipped** | a code invisible to reflection can't be enumerated into the OpenAPI contract — it would reach a user untranslated; the registry keeps the set discoverable + typed end-to-end |
| `SKY0019` | **Error code constant is used**: every `const` on an `*ErrorCodes` registry must be referenced by an `Error`/`Validation` call somewhere in the compilation — the reverse of `SKY0018` | **shipped** | drop a flow and leave its code behind → a dead code still ships in the OpenAPI enum + the i18n catalog; this keeps the registry the exact, live set (no orphans) |
| `SKY0020` | **A `[Journey]` asserts its post-condition**: happy requires an observable-effect assertion; sad requires both rejection and unchanged-state assertion tokens. Error-tier, textual over journey AdditionalFiles | **shipped** | a journey that exists but proves no terminal state is theater |
| `SKY0021` | **A persisted or entity-owned type declares its mark**: a `DbSet<T>` whose `T` is unmarked must be `[Entity]`; a complex member of an `[Entity]` (after one nullable/collection layer) that is neither `[ValueObject]`, `[Entity]`, nor an enum must be `[ValueObject]`. The rung *beneath* SKY0013/SKY0014, which only fire on already-marked types — so an unmarked domain type would otherwise escape every encapsulation check. Does not flag dead/unused types or framework types | **shipped** | the pauta port shipped an anemic `User` (a table, no `[Entity]`) past a green doctor — the omission of the mark was the evasion |
| `SKY0022` | **An endpoint's authorization is a decision, never an omission**: every `[Slice]` carries an explicit posture — `.RequireAuthorization(…)` or `.AllowAnonymous()` — on its own `Map` chain or on the route group the module mounts it on (`app.MapGroup("/x").RequireAuthorization()`). `.AllowAnonymous()` is not a loophole: it is the same decision, made visible and reviewable. A missing `Map` is SKY0001's concern | **shipped** | the classic silent failure — a new endpoint ships open because nobody decided anything |
| `SKY0023` | **An injected `ICurrentUser` is consulted**: a `[Slice]` `Handle` that takes an `ICurrentUser` parameter and never reads it is flagged — the caller was wired in to scope the operation, so an unread parameter is a missing ownership/role/org check, not dead code. Consult the caller or remove the parameter | **shipped** | the signature claims an authorization posture the body doesn't have — the check was meant and then silently dropped |
| `SKY0024` | **Raw SQL never absorbs runtime values as text**: a `*Raw` EF call (`FromSqlRaw`, `ExecuteSqlRaw`/`Async`, `SqlQueryRaw`) whose SQL argument interpolates or concatenates a non-literal is flagged — the SQL-injection shape. The fix is one token: `FromSql`/`ExecuteSql`/`SqlQuery` take the same interpolated string and turn every hole into a `DbParameter`. Constant SQL through `*Raw` stays legal | **shipped** | injection hides in the one raw query an app eventually needs — the safe twin costs nothing |
| `SKY0025` | **A held `Result<T>` is checked before it is unwrapped**: reading `.Value`/`.Error` on a result stored in a local or parameter with no earlier outcome consult in the same member (`IsSuccess`/`IsFailure`, an `is { IsSuccess: … }` pattern, or a `Validation.Collect` fold) is flagged — on the wrong outcome the access throws. Unwrapping *inline* on a fresh construction (`Money.From(10m).Value` in a seed/test) stays legal: it is the deliberate known-valid idiom | **shipped** | the type's number-one misuse — `result.Value` straight through, an exception where an `Error` was supposed to flow |
| `SKY0026` | **Every tracked update/delete declares its concurrency posture**: a slice whose `Handle` mutates or explicitly updates/removes an entity with no visible token (`[Timestamp]`, `[ConcurrencyCheck]`, or `RowVersion`) is flagged. Insert-only rows and entities merely read beside a different write are excluded. Advisory in `skies doctor` because fluent-only configuration is invisible; blocking in `skies gate` | **shipped** | the sample's own `Deposit` raced: two concurrent deposits, one balance silently lost |
| `SKY0027` | **A slice must not materialize an unbounded set**: a `ToListAsync`/`ToList` (or the array twins) ending a `DbSet`-rooted chain — directly or through a queryable local — with no `Take`/`ToPageAsync` on the way is flagged. **Parent-scoped queries are exempt**: a `Where` equating (or `Contains`-matching) a `*Id` member (`s => s.JobId == id` — the steps of ONE job) is bounded by the aggregate's cardinality, and a synthetic `Take(n)` there would document a bound that isn't the real rule; `OrgId`/`TenantId` equality is the tenant scope itself and stays flagged. Warning-tier: legitimately small sets exist, and the fix documents the decision — `.Take(n)` writes the bound down, `ToPageAsync` pages it behind a stable order | **shipped** | hostpoint: list slices served whole tables that paged fine at dev-data scale; pauta's 0.3.0 adoption surfaced ~16 parent-scoped loads (steps of one job, sessions of one user) where the v1 rule over-fired — the exemption is that lesson |
| `SKY0028` | **A paged order needs a unique tiebreaker**: the ordering chain feeding `ToPageAsync` must contain the entity's primary key — a member named `Id`, or the EF-conventional `{Entity}Id` on the queried entity itself (a foreign `*Id` such as `CustomerId` is many-rows-shared and does not count) — else the final sort key is flagged. Warning-tier; a pre-ordered local the analyzer cannot read stays silent | **shipped** | hostpoint: `ListPublicPointReviews` ordered `OrderByDescending(CreatedAt)` with no tiebreaker past a green doctor — rows repeated and vanished between pages once timestamps tied; pauta: 0/34 migrated slices had a tiebreaker before the wave |
| `SKY0030` | **A manifest-declared criterion has a subject-bound AVP proof**: `<Module>.spec.toml`'s `(module, slice, criterion)` obligation is flagged unless an `[AVP(typeof(Slice), "criterion")]` proof with that exact tuple exists. One slice can never borrow another slice's proof | **shipped** | repo-wide id matching let one passing test pay unrelated verification debt |
| `SKY0031` | **Every `[Slice]` declares at least one criterion in its spec manifest**. Together with SKY0030: slice ⟹ criterion ⟹ exact executable proof | **shipped** | optional acceptance verification left most production features unproved |
| `SKY0032` | **Tests must execute**: a test carrying xUnit `Fact/Theory` `Skip`, `SkipWhen`, `SkipUnless`, or `Explicit` is flagged in compiled or AdditionalFiles source, and `skies gate` also rejects every TRX `NotExecuted` outcome. A disabled or conditionally omitted test is never evidence even though `dotnet test` normally exits zero | **shipped** | generated and hand-written skipped tests made a green test process look like verified behavior |
| `SKY0033` | **A journey is one isolated executable E2E proof**: each `*Journey.Tests.cs` method carries `[E2E]`, `[Fact]`/`[Theory]`, and exactly one `[Journey]`; unit/integration categories, stacked subjects and undiscoverable methods do not pay SKY0008 | **shipped** | the least-resistance migration was to relabel an existing unit test or stack many write obligations on one shallow method |

The doctor catches **structural drift**, not logic correctness. Correctness is tests +
review. Expect it to reclaim the *structural* fraction of drift, not 100%.

Beside the SKY* rules the doctor ships a **security floor** for the built-in .NET analyzers
(`buildTransitive/skies.globalconfig`): a curated CA* set — dropped `CancellationToken`
(CA2016), insecure deserialization (CA23xx), broken crypto / disabled cert validation /
deprecated TLS (CA53xx) — raised to error tier. Same removability story: opt out per-project
with `<SkiesSecurityAnalysis>false</SkiesSecurityAnalysis>`, or override a single rule from
your own `.globalconfig` at a `global_level` above 50. The libraries hold themselves to the
same floor via `build/Skies.Framework.Library.props`.

---

## The gate — `skies gate` and the verification matrix

The doctor answers "is the form right?"; the AVP proofs answer "does the behavior hold?".
`skies gate` is the deterministic, change-aware command that answers both and writes the evidence down. Proof
**obligation** is universal; runtime **execution** is selected from Git impact:

1. **Doctor legs** — manifest validation, framework sync, the backend build (every SKY* rule,
   including the SKY0030/SKY0031 bridge) and each manifest-selected frontend harness's lint + typecheck.
   The standalone `skies doctor` keeps warning-tier findings advisory while work is in progress. `skies gate`
   is the final-state boundary: it adds `-warnaserror`, reruns ESLint with `--max-warnings=0`, and invokes
   SKYFE008 in strict mode over each conventional `src/client.gen` and the union of source roots in its
   manifest-declared product topology. A warning therefore remains
   readable during construction but can never be the path by which incomplete work reaches the gate.
   The gate also rejects SKY/SKYFE `#pragma`, `SuppressMessage`, `NoWarn`, downgraded analyzer severity,
   generated-code impersonation, inline `eslint-disable`, and disabled-rule configuration. Its ESLint leg
   independently forces every rule exported by the installed harness, so an indirect config expression cannot
   hide one either. The contract must be satisfied, not silenced.
2. **Backend proof leg** — the default/`--affected` form expands changed slices and test files to their unit,
   `[AVP]`, and `[Journey]` classes and supplies the resulting filter itself. A Roslyn syntax graph follows changed
   shared types and extension methods through every direct and transitive C# consumer, so an entity or service
   selects the slices and test classes that actually depend on it instead of paying for the whole solution.
   Ambiguous names conservatively add edges; a production file with no reachable proof remains fail-closed.
   `--staged --fast` roots that selection in the index; `--full` executes every test. A caller-authored `--filter`
   is rejected, and any selected skipped/not-executed result makes the gate red.
3. **Frontend proof legs, selected by product role** — every product `core` runs `npm run test`
   over the non-Assay partition plus direct `assay verify` over `*.assay.test.*`; every executable `frontend`
   runs those same legs plus the strict `skyfe-e2e-doctor` and `npm run test:e2e`. The workspace-level
   `skyfe-feature-e2e` leg requires every ViewModel to resolve to one of those surface flows, every backend slice
   hook consumed by a frontend data door to be named by the linked flow, and subject-bound happy+sad paths for
   every visible feature. A backend slice with no frontend consumer remains backend-only. Product `website`
   declarations are surfaces too; validation independently inventories ViewModel/flow-bearing packages, so deleting
   a product key cannot shrink the gate. The filename partition makes every selected test execute exactly
   once: AVP is a real gate leg, not a duplicate second Vitest pass. `test:e2e` must contain the unfiltered full
   runner rather than an opaque wrapper or file/grep selector, and the doctor compares every manifest `spec/case`
   with Playwright's collected inventory before execution. A missing or placeholder script, narrowed config,
   manifest, runner, terminal assertion, seed, or real execution is a failure; there is no omitted-proof tier.
   Affected execution uses ViewModel names, flow `features`, and `backendSlices` to select co-located Assays and the
   exact Playwright/Maestro specs. Ambiguous/shared frontend changes widen to the whole package or workspace.
4. **The matrix** — declarations (`*.spec.toml`) × proof sites
   (`[AVP(typeof(Slice), "id")]`) × test verdicts,
   joined per module: criterion → proof → verdict, plus the findings the gate refuses to swallow —
   a declared criterion with no proof (the gap), a proof no manifest declares (creep), a
   slice declaring nothing, a proof that never reached a decision (a skip is never a
   pass), a malformed manifest. In affected runs, valid unselected rows are explicitly `not-affected`; they are
   non-blocking inventory rows and never counted as passes.

Execution modes are closed and framework-owned:

- `skies gate` — complete transitive proof closure for committed and local working-tree changes;
- `skies gate --affected --base <revision>` — push/PR closure frozen to `<revision>...HEAD`, excluding local-only and untracked files that cannot enter that review;
- `skies gate --staged --fast` — index-rooted pre-commit feedback; exhaustive fallbacks and browser/device execution
  are deferred to the repository's selected authority boundary;
- `skies gate --affected --base <revision> [--fast]` — pre-push review over the commits being sent. CI-authority
  repositories use `--fast`; local-authority repositories omit it and own the authoritative affected verdict;
- `skies gate --full` — exhaustive audit, mandatory before release and optionally configured for main/manual/nightly.

Missing Git ancestry makes a scoped gate incomplete (exit 2); restore the comparison before retrying, or request
an explicit full audit at the release boundary. If production impact cannot be mapped or runtime-wide build/test
infrastructure changes, the selector widens to full. Gate-control files (`.config/dotnet-tools.json`, `lefthook.yml`, and checked
workflows) are different: the doctor validates their contract, but changing the fiscal does not execute unrelated
application proofs. Application annotations and arbitrary test filters never participate.

Authoritative and full `check`/`gate` commands allow one automatic attempt per checkout. A failed or interrupted
attempt blocks the next broad run, even after edits or a scope change. An exclusive lease also prevents overlapping
broad runs. Staged and fast checks remain available for diagnosis. Stop and check whether you are in a loop before
retrying: inspect the first failure, correct its cause and verify the correction with a focused test. Then supply
`--retry-review <json-file>` with `PreviousAttemptId`, `Diagnosis`, `Correction`, and `FocusedVerification` strings.
The previous attempt ID is in the receipt printed by the command. A review authorizes one further attempt and cannot
be replayed after another failure; it records the operator's evidence rather than independently certifying it.
No proof is waived. Successful completion permits subsequent work. Attempt state lives in Git metadata under
`skies-verification`; do not delete it to bypass a failed run. This guard controls these CLI commands, not model
tokens or arbitrary commands invoked outside Skies.

The C# dependency graph resolves top-level types and qualified references: nested `Input`/`Output` names and
namespace segments never join unrelated slices. A module specification selects that module; a transitive proof
selects its own test class without recursively selecting every other proof of every subject it mentions.
Long backend selections use one standard runsettings filter, never an unfiltered run or overlapping batches.
Generated TypeScript clients propagate changed runtime exports through actual imports; erased type changes
remain covered by the universal typecheck. Shared client imports cross declared package boundaries using each
package's TypeScript configuration. Uncertain generated impact falls back to the owning package.
Feature-local translations select their feature and locale proofs. Flow matching uses the original affected
features, and Playwright executes a deduplicated native test list containing the mapped cases, one worker.
Doctor concurrency and Vitest workers are capped at two; .NET build/test defaults to two MSBuild nodes.
Affected console output summarizes passing and
unaffected rows while preserving every finding; full audits retain the complete matrix.
Native Node unit scripts receive Node file selection and concurrency arguments, with executable JUnit evidence,
instead of Vitest flags. Selected backend runs require valid non-empty receipts, and a full Flutter library
requires an executable unit or Assay suite. A process exit of zero without evidence is not a passed proof.

An explicit `--full` audit writes the canonical artifacts at the workspace root: **`VERIFICATION.md`**
(human — commit it; a reviewer reads the exhaustive proof state without running anything) and
**`VERIFICATION.json`** (machine-readable attestation). Staged and affected gates print the same scoped matrix
without replacing those files, so hooks and CI never dirty a checkout or downgrade the committed full audit to a
partial snapshot. The exit code IS every gate's verdict: 0 only when every selected leg and the matrix hold.

### The gate travels with the scaffold — born gated, like slices and modules

A gate nobody is forced to run is opt-in by human memory — and a pilot proved the failure mode:
a whole module landed on `main` with every one of its tests red, silently, because nothing watched
the push. So the wiring is part of the supply, not an exercise for each app:

- **`skies new` scaffolds the enforcement**: `.github/workflows/ci.yml` runs the affected gate on every push and PR
  and runs the full audit on version tags or manual dispatch. `lefthook.yml` runs the staged-fast gate before commit
  and bounded base-relative feedback before push. `.config/dotnet-tools.json` pins the CLI version used by hooks
  and CI.
- An app is **born gated** exactly as it is born with slices, modules and the doctor: red work
  cannot reach `main` unnoticed. Retrofitting an existing app is the same three files.
- Manifest validation requires exactly one authoritative affected boundary without `--fast`: either a checked
  pull-request workflow or a base-relative local pre-push hook. It also requires an explicit `--full` package
  command or release/reusable/manual workflow. CI authority is the scaffold default; local authority avoids a
  duplicated remote run when one development machine is also the delivery boundary.

### Semantic memory travels with the scaffold

Skies projects start with Codebase Semantic Memory as the shared home for NYA,
RTW, WTW, and NWC:

- `csm.toml` declares the common storage root. Skies scaffolds `.skies/csm/`,
  with one child owned by each independent tool. Personal judge selection and
  disposable indexes remain ignored inside that tree.
- `dotnet tool run skies <nya|rtw|wtw|nwc> <args...>` resolves the
  framework-pinned CSM release. CSM verifies, installs, and locks the native
  tool versions outside the repository. No Rust toolchain or global install is
  part of project setup.
- Root agent instructions require recall at task start and whenever scope or
  context changes, scar-driven review before accepting a versioned
  specification, remember only after a real reusable correction, and check
  before completion.
- `nya replay` is reserved for explicit corpus maintenance or evaluation. It
  validates historical correction patches against their scars without
  executing an agent or claiming a prevention rate.
- Local pre-commit and pre-push hooks run the matching uncommitted or
  base-relative recurrence check. CI does not select a model or credential on
  behalf of the team; an organization may add its own ephemeral judge after
  configuring that trust boundary.
- `skies doctor` validates the versioned CSM surface without reading personal
  configuration or downloading binaries.

The split is deliberate. Skies deterministically proves code shape and
executable behavior. CSM coordinates the repository-specific semantic layers;
each underlying tool remains independently usable outside a Skies project.

### Declaring criteria — born closed, not caught later

The bridge is cheapest when the slice is born with it:

```
skies criteria list                          # the catalog menu — and what Assay.Net can actually run
skies criteria suggest CreateCheckout        # ranked archetype families for a slice, with the matched words
skies g slice Payments Charge --verify idempotency-key-honored,gate-enforced-on-submission
```

`--verify` declares the ids in `Modules/<M>/<M>.spec.toml` (creating the manifest with the house
header, or surgically merging into an existing one — human edits survive) and scaffolds the
co-located `<Slice>.Avp.Tests.cs`: one test per criterion carrying
`[AVP(typeof(Slice), "id")]`, already wired to
the right Assay.Net archetype, **red by design** until the subject factory boots the real endpoint.
Obligation and proof ship in the same change-set — deferral is exactly the gap the bridge exists to
close. Calibrate before trusting a green: the same verifier must FAIL an escape variant (the
sample-app's `Withdraw.Avp.Tests.cs` shows the full shape).

---

## The self-harness — framework-dev only, never shipped

The libraries hold *themselves* to a standard, the way the Rails repo does. This is the
**self-harness** (`Skies.Framework.SelfHarness`): analyzers that run only on Skies' own source.

It is **closed to this repo**: `IsPackable=false`, referenced with
`ReferenceOutputAssembly="false"`, never packaged, and **never part of the production CLI or
the user-facing `Skies.Framework.Doctor`**. It is the `skies` vs `skies-dev` split — framework-dev
tooling stays out of the published surface, always.

| Rule | Enforces | Applies to |
|------|----------|------------|
| `SKYSELF001` | File at or under 500 lines | `Skies.Framework.*` libraries |
| `SKYSELF002` | No tracking codes or scratch markers in comments (TODO, FIXME, HACK, XXX, capital-letter/number codes) | `Skies.Framework.*` libraries |
| `CS1591` (built-in) | Every public member carries XML documentation | `Skies.Framework.*` libraries |

Settings live once in `build/Skies.Framework.Library.props`, imported by every `Skies.Framework.*` project. The
bar is code a Microsoft .NET MVP would read and be proud of: gold-standard docs, no junk,
small files. The **user's app is not** held to these — `SKYSELF*` never touches a generated
user project.

---

## Scope — and non-goals

**In:** the standard project shape, the doctor, the ai-context discipline, the thin wire
(`Result<T>`, `[Slice]`, `[ValueObject]`, `[Entity]`), a slice scaffold (`rails g`-style,
planned), a knowledge-graph dump for the LLM (planned).

**Out (non-goals), by decision:**
- **No source-gen of behavior.** Plumbing only, if ever — and not in v0. (It is a
  mini-compiler: the source-gen vector.)
- **No vendor adapters in the core.** MercadoPago/Twilio/etc. are written *following* the
  component standard, in separate repos — the kit ships the standard, not the plugins.
- **No source-gen of UI behavior, no realtime *on by default*, no multi-app sprawl.** (The
  aerocoding-2 failure modes — designed out.) *Nuance:* the frontend is scaffolded-once-and-owned
  + enforced, never re-generated; real-time is **opt-in** via `skies g hub` (see
  §"Real-time — hubs"). The failure mode is the sprawl/source-gen, not the capability.
- **No runtime framework you inherit from.** Conventions + analyzer, not base classes.

When a proposal smells like capability instead of convention+enforcement, it is a scope
violation. Reject in line.
