# sample-app — the canonical Skies example

A small wallet product that shows the Skies 5 shape end to end:

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
- **`frontend/core`** — the platform-agnostic layer: ViewModels (the single data door), Views, i18n, and the
  generated client. **`frontend/web`** and **`frontend/mobile`** are the app's own `ui/` components per platform.

## Run it

```bash
dotnet test examples/sample-app/backend/Sample.Tests
skies proof status              # from examples/sample-app
skies proof verify --all
npm --prefix frontend-sdk run check   # typechecks, lints, and tests the frontend sample
```
