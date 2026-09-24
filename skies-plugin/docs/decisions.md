# Skies — Key decisions (ADR digest)

- **Skies 5 — evidence over apparatus**: the doctor enforces architecture only; features are accepted by a receipt
  in `.specs/<id>/` (failure modes first, black-box E2E, red then green); no gates, no hooks, no rule that demands
  a test, tag, or manifest. The Skies 4 gate grew larger than the framework and taught agents to satisfy the checker.

- **Frontend harness**: MVVM over plain custom hooks (no classes/observables/two-way binding); orval stock wrapped,
  never a bespoke compiler; typed slice-hooks via OpenAPI so completeness is enforced by tsc; ViewModels scaffolded
  once then owned; the ESLint plugin is optional and doctor-removable.

- **Unmarked domain type (SKY0021)**: `DbSet<T>` with unmarked T requires `[Entity]`; complex members of entities
  require `[ValueObject]`. Omission of the mark was the evasion (a pilot shipped an anemic `User`).

- **Monorepo**: ordinary backend and frontend packages named in `[products.*]`; runners in `[runners.*]`.

- **Core mission**: scaffolding + the doctor + AI-context discipline (ctx.md) + spec receipts. Apps bring their
  own libraries, design system, and business logic; vendors live in plugins, not in the framework.
