# sample-app — the canonical Skies example

Demonstrates the Skies monorepo shape end to end (the same architecture in `docs/MONOREPO-ARCHITECTURE.md`):

- **[`Skies.toml`](Skies.toml)** — makes this folder a self-contained Skies app: its backend and the `api` runner
  that executes spec E2E. The frontend below is a framework-owned fixture, not a deployable surface pretending to
  have a browser/device runner.
- **`.specs/`** — the Wallets features as specs (`0001-deposit`, `0002-withdraw`, `0003-wallet-reads`): failure
  modes in `spec.md`, black-box E2E in `e2e/`, and a `receipt.json` with evidence. They were written after the code,
  so each carries a `red.patch` that stubs its slices back to "not implemented" for the red run.
- **`backend/`** — the .NET API (`Sample.Api`) + its co-located tests (`Sample.Tests`), vertical slices under the
  `SKY*` Roslyn analyzers.
- **`frontend/core/`** — the **platform-agnostic** layer: the ViewModel (the single data door), the
  design-system-driven View (it reaches the UI only through `@/ui`, never `react-native` directly), i18n, and the
  generated client. It has **no** `react-native` / `react-dom` dependency — that is what makes it shareable.
- **`frontend/web/`** — the **web** design-system impl of `@/ui` (react-dom primitives).
- **`frontend/mobile/`** — the **mobile** design-system impl of `@/ui` (React Native primitives).

The View is written **once** (agnostic) in `core`; only the `@/ui` implementation differs per platform.

## Verification

- **Backend** — `dotnet test examples/sample-app/backend/Sample.Tests` (32 tests, green).
- **Specs** — from `examples/sample-app/`: `skies proof status` (hashes only), `skies proof verify --all` (reruns
  green), or `skies proof record <id>` (red with the spec's `red.patch`, then green).
- **Frontend core + web** — the framework's vitest (run `npm run check` in `frontend-sdk/`) mounts the core's View
  against the **web** `@/ui` in jsdom — wired, not mocked.
- **Mobile** — built with the consumer's Expo/Metro toolchain (which provides `react-native`), like a real app's
  mobile target; it is not part of the framework's web-only check.
- **Full static typecheck + SKYFE lint across `core`/`web`** runs once skies-framework adopts a root npm workspace (a
  shared `node_modules` so the example — a sibling of `frontend/` — resolves the spine + deps). The example's
  `frontend/tsconfig.json` is already wired for it. That root workspace is itself the monorepo this example shows.
