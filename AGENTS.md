# Skies — operating manual for AI agents

Skies is the **opinionated convention bundle for .NET, React, and Flutter**: a standard vertical-slice and MVVM
architecture, architecture-only doctors, scaffolders, and spec receipts, so an LLM has less to decide and what it
writes is checked. It is the Rails mindset (convention over configuration, semantic density), not the Rails
mechanism: no runtime metaprogramming, no language. Reference codebase: `rails/rails`.

## The three laws

1. **Stranger-maintainable.** Output is plain, idiomatic C#, TypeScript, or Dart that a developer who never heard of
   Skies can read and maintain.
2. **Doctor-removable.** Remove the Roslyn analyzer, the ESLint plugin, or the Flutter rules and the app still
   compiles and runs; only enforcement is lost.
3. **Evidence over apparatus.** A feature is accepted by a reproducible receipt in its spec folder. The framework
   ships no gate, no hook, no check that audits the agent, and no rule that demands a test, tag, or manifest.

A proposal that breaks one of these — hidden source generation of behavior, a DSL, a base class, magic discovery,
a new gate — is out. Reject it in line.

## Repository layout

```
cli/                               The `skies` binary (Rust): scaffolders, doctor, spec/proof engine, migrate.
  templates/                       Embedded templates: app (skies new), dotnet generators, react, flutter.
src/Skies.Framework.*/             .NET runtime packages: Abstractions, AspNetCore, Auth, EF Core, ports, Testing.
analyzers/Skies.Framework.Doctor/  SHIPPED SKY#### Roslyn rules (architecture only) + the CA* security floor.
analyzers/Skies.Framework.SelfHarness/  FRAMEWORK-DEV ONLY SKYSELF#### rules on our own .NET code. Never shipped.
frontend-sdk/packages/             @skiesjs/react (spine) and @skiesjs/eslint-plugin (SKYFE### rules): React for the web.
flutter-sdk/packages/skies_flutter/  The Flutter spine: Flutter for mobile, and for the web too.
examples/sample-app/               The reference app: Wallets backend, React web package, and .specs/ with receipts.
docs/                              Conventions (backend, frontend, Flutter), monorepo, migration, decisions.
skies-plugin/                      The agent plugin: the skies-sdd skill, context, and docs/ synced from docs/.
.specs/                            This repository's own specs (the Skies 5 plan lives in 0008).
```

Ground every convention fact in `docs/CONVENTIONS.md`, `docs/FRONTEND-CONVENTIONS.md`, and
`docs/FLUTTER-CONVENTIONS.md`, never memory.

## The bar for code you write here

- **Files at or under 500 lines.** Past it, extract a concern (`SKYSELF001` on .NET; the same rule by review for
  Rust, TypeScript, and Dart).
- **Documentation that explains why.** Every public .NET member carries XML docs (`CS1591` is an error); Rust items
  carry doc comments. Lead with why, not what.
- **No junk comments.** No TODO/FIXME/HACK/XXX, no tracking codes, no materialized agent thoughts (`SKYSELF002`).
- **Tests prove behavior.** Framework library tests under `tests/` prove each package's public API. Anything an
  application sees (the sample, generated apps) is proven by specs: every such test lives in a `.specs/` folder
  (`SKY0029`, `SKYFE036`, `SKYFL036`).

If a build fails on `SKYSELF*` or `CS1591`, fix the code; never suppress the rule.

## Build and test

```
cargo test && cargo clippy -- -D warnings                 # the skies binary
dotnet build Skies.Framework.slnx && dotnet test Skies.Framework.slnx
npm --prefix frontend-sdk run check                       # typecheck, lint, tests
(cd flutter-sdk/packages/skies_flutter && flutter analyze && flutter test)
tools/auth-smoke.sh                                       # render auth into a new app, build doctor-clean, run its specs
tools/sync-plugin-docs.sh                                 # after editing docs/*CONVENTIONS.md or the CLI help
```

Run what your change touches. Leave every affected workspace green.

- A template change moves the generator snapshots: re-bless with `SKIES_BLESS=1 cargo test --test generators` and
  review the fixture diff. `cli/templates/app/.claude/skills/skies-sdd/SKILL.md` is a verbatim copy of
  `skies-plugin/skills/skies-sdd.md`; edit both.
- The sample is an app like any other: its web specs' stand-in backend is `examples/sample-app/.specs/web.setup.ts`,
  not a `frontend-sdk` file. An edit to the sample's code or specs can stale its receipts (`skies proof status` from
  `examples/sample-app/`).

## The doctor vs the self-harness

- **`SKY*` (`Skies.Framework.Doctor`)** runs on the user's code and ships. **`SKYSELF*`** runs on ours, is
  `IsPackable=false`, and never enters a published artifact.
- A new doctor rule must enforce architecture and be born from observed drift in a real application. A rule that
  would require a test, a tag, or a manifest to exist is out by the third law.

## Scope discipline

The cautionary tales are concrete. **The predecessor language** died owning a compiler. **Aerocoding** died from
scope explosion. **Skies 4** grew a verification gate larger than the framework it verified. So:

- No source generation of behavior. No vendor adapters in core. No runtime framework to inherit from.
- `[Slice]` stays a pure marker; `.ctx.md` stays prose.
- New tooling goes into the Rust binary, is invoked on purpose, and never blocks by default.

## Package-first releases

Applications consume Skies only as versioned packages, never source copies. Framework-shaped code lands here first.
Every package (NuGet, npm, pub, the binary) shares the version in `build/Skies.Framework.Library.props` and
`Cargo.toml`, and they are released together.

## Git discipline

- Stage specific files (`git add <path>`), never `-A` or `.`.
- One commit per concern; lowercase, present-tense imperative messages.
- No `--force`, no history rewrites.
