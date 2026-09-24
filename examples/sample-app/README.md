# sample-app — the canonical Skies example

A small wallet product that shows the Skies 5 shape end to end:

- **[`Skies.toml`](Skies.toml)** — makes this folder a self-contained Skies app: its backend and the `api` runner
  that executes spec E2E.
- **`backend/Sample.Api`** — the .NET API: a `Wallets` module with `Deposit`, `Withdraw`, `GetBalance`, and
  `ListWallets` slices, a rich `Wallet` entity, the `Money` value object, and the module's `Wallets.ctx.md`. The
  `SKY*` analyzers run in its build.
- **`backend/Sample.Tests`** — the test runner: it boots the real app through `SkiesWebTest<Program>` and compiles
  the spec E2E under `.specs/*/e2e`, plus the co-located `Money.Tests.cs` (an isolated value object, the one kind
  of unit test Skies recommends).
- **`.specs/`** — one folder per feature: `0001-deposit`, `0002-withdraw`, `0003-wallet-reads`. Each has its
  failure modes in `spec.md`, black-box HTTP tests in `e2e/`, and a `receipt.json` showing every failure mode
  failing on the red revision and passing on green. These specs were written after the code, so each carries a
  `red.patch` that stubs the slice and reproduces "feature not implemented".
- **`frontend/web`** — the React web package: one folder per feature (the ViewModel as the single data door, the
  View, the i18n catalog), the generated client (`src/client.gen`), the i18n instance, and the app's own `ui/`
  kit. Its cases live in the web specs (`0006`–`0008`).

## Run it

```bash
dotnet test examples/sample-app/backend/Sample.Tests
cd examples/sample-app
skies proof status              # hashes only
skies proof verify --all
npm --prefix frontend-sdk run check   # typechecks, lints, and tests the frontend sample
```
