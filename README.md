<p align="center">
  <img src="assets/logo.png" alt="Skies albatross engineer mascot" width="400">
</p>

<h1 align="center">Skies</h1>

<p align="center"><strong>Convention over configuration for AI-built .NET, React, and Flutter applications.</strong></p>

<p align="center">
  <a href="#getting-started">Getting Started</a> |
  <a href="#specs-and-receipts">Specs</a> |
  <a href="#the-convention-model">Conventions</a> |
  <a href="#packages">Packages</a> |
  <a href="https://skies.build">Website</a>
</p>

<p align="center">
  <a href="https://github.com/lucasrgt/skies/actions/workflows/ci.yml"><img src="https://github.com/lucasrgt/skies/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI"></a>
  <a href="https://www.nuget.org/packages/Skies.Framework"><img src="https://img.shields.io/nuget/v/Skies.Framework?style=flat-square&label=.NET" alt="Skies.Framework on NuGet"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-2EA44F?style=flat-square" alt="MIT License"></a>
</p>

AI coding agents are good at producing locally plausible code. They are less reliable at keeping a system's
architecture and at proving that a feature survives its unhappy paths.

Skies narrows the decision space and asks for evidence, nothing more. It gives ordinary applications one visible
shape per operation, scaffolders that start in that shape, doctors that reject structural drift, and a spec folder
per feature whose receipt shows the feature's failure modes fail before the change and pass after it. The output is
plain C#, TypeScript, and Dart that a developer who has never heard of Skies can maintain.

<table>
<tr><td><b>One operation shape</b></td><td>A slice keeps its input, output, handler, and transport mapping together. A frontend feature is one ViewModel and one View.</td></tr>
<tr><td><b>Explicit architecture</b></td><td>Modules, routes, dependencies, and error registries stay visible in normal code. No reflection discovery, no base classes.</td></tr>
<tr><td><b>Architecture doctors</b></td><td>Roslyn, ESLint, and Flutter rules reject structural drift. They never demand tests, tags, or manifests.</td></tr>
<tr><td><b>Evidence, not gates</b></td><td>Each feature has <code>.specs/&lt;id&gt;/</code>: failure modes written first, black-box E2E, and a receipt. Nothing runs in a hook unless you add one.</td></tr>
<tr><td><b>One fast binary</b></td><td>Scaffolders, the doctor, and the proof engine are a single Rust executable.</td></tr>
</table>

---

## Getting started

Install the CLI (a single binary):

```bash
npm install -g @skiesjs/cli      # or: cargo install skies-cli
skies new Billing
cd Billing
```

### .NET and ASP.NET Core

The `Skies.Framework` meta-package brings the runtime convention and the Roslyn doctor in one reference. The
composition root stays a short index:

```csharp
var builder = WebApplication.CreateBuilder(args);

builder.Services.AddSkies();
builder.Services.AddPlatform(builder.Configuration);
builder.Services.AddModules(builder.Configuration);

var app = builder.Build();

app.UseSkies();
app.UsePlatform();
app.MapModules();

app.Run();
```

A feature is one static slice. Its handler is HTTP-agnostic; its map is the thin transport boundary:

```csharp
[Slice]
public static class CreateInvoice
{
    public sealed record Input(Guid CustomerId, decimal Amount);
    public sealed record Output(Guid InvoiceId);

    public static async Task<Result<Output>> Handle(Input input, AppDb db, CancellationToken ct)
    {
        var amount = Money.From(input.Amount);
        var validation = new Validation()
            .Require(input.CustomerId, "customerId", BillingErrorCodes.CustomerRequired)
            .Collect("amount", amount);
        if (validation.Failed)
            return validation.ToError();

        var invoice = Invoice.Create(input.CustomerId, amount.Value);
        db.Invoices.Add(invoice);
        await db.SaveChangesAsync(ct);
        return new Output(invoice.Id);
    }

    public static void Map(IEndpointRouteBuilder app) =>
        app.MapPost("/invoices", async (Input input, AppDb db, CancellationToken ct) =>
                (await Handle(input, db, ct)).ToHttp())
            .RequireAuthorization()
            .WithName(nameof(CreateInvoice));
}
```

Generate the standard shapes instead of rebuilding them from memory:

```bash
skies g module Billing
skies g slice Billing CreateInvoice
skies g entity Billing Invoice
skies g vo Money
skies g crud Billing Invoice
skies g auth            # + auth:otp, auth:oauth, auth:email
skies g hub Billing InvoiceUpdates
skies doctor
```

### React (web)

```text
clients/web/src/create-invoice/
  CreateInvoice.view.tsx
  CreateInvoice.viewModel.ts
  create-invoice.i18n.ts
```

React is the web body. The ViewModel is a render-agnostic hook and the only data door; it composes the generated
query hooks. The View renders. `@skiesjs/react` provides the small spine (`AsyncState`, `Resource`, session, guards, paging, forms) and
`@skiesjs/eslint-plugin` enforces the `SKYFE###` architecture rules. Styling and components are the app's:
`g web-app` starts the package with a small kit it owns.

```bash
skies g web-app Web --path clients/web     # Vite + React + TanStack Router, declared in Skies.toml and CI
npm install --prefix clients/web
dotnet build                               # writes the OpenAPI contract the client and screens read
cd clients/web && skies g client
skies g feature Invoices                   # a list over ListInvoices
skies g feature CreateInvoice --kind form  # a form over CreateInvoice's inputs
npm run build && npm run lint              # skies doctor runs the lint and typecheck too
```

### Flutter

```text
lib/features/account/wallets/
  wallets_view.dart
  wallets_view_model.dart
lib/l10n/features/
  wallets_{pt_BR,en,es}.arb
```

Flutter is the mobile body, and a supported web body too: a product that wants the same components on every
surface ships its phone apps and its web app from one Flutter codebase. `skies_flutter` supplies the matching spine: async composition, session, guards, routing, forms, mutation
defaults, localized errors, and paging. `skies g client` wraps stock OpenAPI Generator `dart-dio`; `skies doctor`
runs the `SKYFL###` rules natively.

```bash
skies g flutter-app App --path clients/app   # flutter create, plus the pinned spine, i18n, Skies.toml, and CI
cd clients/app && skies g feature Wallets
```

---

## Specs and receipts

A feature is accepted by evidence in its spec folder:

```text
.specs/0012-cancel-reservation/
  spec.md          behavior + failure modes (FM-1..n), written before the code
  e2e/             the spec's cases; a case named "FM-2: …" covers FM-2
  receipt.json     red (every FM fails before the change) and green (every FM passes after)
  evidence/        small artifacts a case chose to save (full reports stay local in evidence/raw/)
```

```bash
skies spec new cancel-reservation      # write the failure modes, then the E2E, then the code
skies proof run 0012                   # run the spec's E2E once, per failure mode; writes nothing
skies proof record 0012                # red on the merge-base, green on the working tree, write the receipt
skies proof impact <paths>             # the specs a change reaches, before you make it
```

CI owns green: it runs every spec's cases on every push. The receipt records only what CI cannot, that the cases
failed before the feature and passed with it.

Runners live in `Skies.toml`: any command that runs one spec's `e2e/` folder and writes a JUnit or TRX report.
The engine does not care whether that is xUnit, Playwright, Vitest, Maestro, or `integration_test`.

Every test lives in a spec. There is no other home for one: an isolated system (a value object, a parser) gets its
own spec whose `e2e/` holds isolated cases, and the doctors flag a test anywhere else (`SKY0029`, `SKYFE036`,
`SKYFL036`).

---

## The convention model

### Three laws

1. **Stranger-maintainable.** Output is ordinary, idiomatic application code. Reading, debugging, or extending it
   needs no Skies knowledge.
2. **Doctor-removable.** Remove the analyzers and lint plugins and the application still compiles and behaves the
   same. Only enforcement disappears.
3. **Evidence over apparatus.** Features carry reproducible receipts; the framework installs no agent-process gates.

These laws exclude hidden source generation of behavior, DSLs, base-controller runtimes, reflection discovery,
generated UI behavior, and framework-owned business logic.

### One feature, one readable unit

| Concern | .NET | React | Flutter |
| --- | --- | --- | --- |
| Contract | nested `Input` / `Output` records | generated client types | generated `dart-dio` models |
| Behavior | static `Handle` returning `Task<Result<T>>` | render-agnostic `use…Model` hook | `ChangeNotifier` ViewModel |
| Boundary | thin `Map` using Minimal APIs | thin View consuming one ViewModel | thin Widget consuming one ViewModel |
| Context | one `<Module>.ctx.md` | feature naming and i18n | feature naming and ARB |
| Evidence | `.specs/<id>/` | `.specs/<id>/` | `.specs/<id>/` |

Handlers use the application's `DbContext` directly: no repository-per-entity or unit-of-work facade. Expected
failures travel as `Result<T>` with stable, namespaced error codes. Modules own their routes, services, errors,
context, and writes, and refer to other modules by id.

---

## Packages

All packages share one version and are released together.

| Package | Purpose |
| --- | --- |
| **`Skies.Framework`** (NuGet) | Front door: runtime framework plus the Roslyn doctor |
| `Skies.Framework.Abstractions` | `Result<T>`, `Error`, `Validation`, `Page<T>`, and marker attributes |
| `Skies.Framework.AspNetCore` | `AddSkies`, `UseSkies`, slice-aware OpenAPI, `Result` to HTTP |
| `Skies.Framework.EntityFrameworkCore` | Ordered, bounded EF Core pagination |
| `Skies.Framework.Auth` | JWTs, password hashing, refresh sessions, verification tokens, and cookies |
| `Skies.Framework.Identity`, `.Mail`, `.Sms`, `.Storage` | Provider-independent ports |
| `Skies.Framework.Testing`, `.Testing.InMemory`, `.Testing.Postgres` | Real-host test boot and isolated databases |
| `Skies.Framework.Doctor` | The `SKY####` analyzers, for analyzer-only use |
| `@skiesjs/react` (npm) | Render-agnostic React spine |
| `@skiesjs/eslint-plugin` (npm) | `SKYFE###` rules |
| `skies_flutter` (pub.dev) | Flutter spine |
| `@skiesjs/cli` (npm) / `skies-cli` (crates.io) | The `skies` binary |

---

## Documentation

- [.NET conventions, specs, and the `SKY####` catalog](docs/CONVENTIONS.md)
- [Frontend conventions and the `SKYFE###` catalog](docs/FRONTEND-CONVENTIONS.md)
- [Flutter conventions and the `SKYFL###` catalog](docs/FLUTTER-CONVENTIONS.md)
- [Monorepo architecture and `Skies.toml`](docs/MONOREPO-ARCHITECTURE.md)
- [Migrating to Skies 5](docs/MIGRATING-TO-SKIES.md)
- [Why Skies 5 removed the gates](docs/decisions/skies-5-evidence-over-apparatus.md)
- [Sample application](examples/sample-app/)

---

## Build and contribute

Requirements: .NET 10 SDK, Rust stable, Node.js 24, Flutter stable, Java 21 for OpenAPI Generator, and Docker for
the PostgreSQL tests.

```bash
cargo test && cargo clippy -- -D warnings
dotnet build Skies.Framework.slnx && dotnet test Skies.Framework.slnx
npm --prefix frontend-sdk ci && npm --prefix frontend-sdk run check
(cd flutter-sdk/packages/skies_flutter && flutter analyze && flutter test)
```

The framework's own self-harness treats missing public XML documentation and oversized files as build failures.
Fix the code or documentation; do not suppress the rule.

---

## License

MIT. See [LICENSE](LICENSE).
