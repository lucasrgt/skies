# sample-app — the canonical Skies example

A small wallet product that shows the Skies 5 shape end to end:

- **[`Skies.toml`](Skies.toml)** — makes this folder a self-contained Skies app: its backend and the `api` runner
  that executes spec E2E.
- **`backend/Sample.Api`** — the .NET API: a `Wallets` module with `Deposit`, `Withdraw`, `GetBalance`, and
  `ListWallets` slices, a rich `Wallet` entity, the `Money` value object, and the module's `Wallets.ctx.md`. The
  `SKY*` analyzers run in its build.
- **`backend/Sample.Tests`** — the test runner: it boots the real app through `SkiesWebTest<Program>` and compiles
  the spec cases under `.specs/*/e2e` and nothing else.
- **`.specs/`** — one folder per feature: the backend's `0001-deposit` to `0005-openapi-contract` and
  `0009-transfer` (`0004-money` is an isolated value object with isolated cases), the web's `0006`–`0008` and
  `0010`. Each has its failure modes in `spec.md`, its cases in `e2e/`, and a `receipt.json` showing every
  failure mode failing on the red revision and passing on green. Specs written after the code carry a `red.patch` that
  stubs the feature and reproduces "not implemented"; `0009-transfer` was written first, so its red is a revision.
- **`frontend/web`** — the React web package: one folder per feature (the ViewModel as the single data door, the
  View, the i18n catalog), the generated client (`src/client.gen`), the i18n instance, and the app's own `ui/`
  kit. Its cases live in the web specs (`0006`–`0008`, `0010`).

## Run it

```bash
dotnet test examples/sample-app/backend/Sample.Tests
cd examples/sample-app
skies proof run 0001            # one spec's E2E, once, per failure mode; writes nothing
skies proof impact backend/Sample.Api/Modules/Wallets   # the specs a change there reaches
npm --prefix ../../frontend-sdk run check   # typechecks, lints, and tests the frontend sample
```
