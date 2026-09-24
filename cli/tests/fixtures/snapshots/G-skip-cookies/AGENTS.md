# Golden — operating manual for AI agents

This is a **Skies** app: a vertical-slice .NET backend, a doctor that checks its architecture at build time, and
one `skies` CLI. The conventions exist so you have less to decide and what you write is checked.

## The three laws

1. **Stranger-maintainable.** Write plain, idiomatic code a developer who never heard of Skies can maintain.
2. **Doctor-removable.** Remove the analyzers and the app still compiles and runs; only enforcement is lost.
3. **Evidence over apparatus.** A feature is done when its spec folder holds a receipt: failure modes written
   first, black-box E2E that fail before the change and pass after. No tags, manifests, or gates.

## How to deliver a feature

Follow the `skies-sdd` skill (`.claude/skills/skies-sdd/SKILL.md`): write the failure modes and stop for the
human, write the E2E and watch them fail, implement until they pass, then record the receipt. Every test lives in
a spec's `e2e/` folder, never beside the code.

The generators, runnable from the app root:

```
skies g module <Name>              # <Name>Module.cs + <Name>.ctx.md, wired into Modules/Modules.cs
skies g slice <Module> <Name>      # one slice file, mapped under the module's route group
skies g entity <Module> <Name>     # an encapsulated [Entity]
skies g crud <Module> <Entity>     # list/lookup/create/update/delete slices over it, DbSet registered
skies g vo <Name>                  # a value object in BuildingBlocks/
skies g auth                       # the Account module (auth:otp, auth:oauth, auth:email add flows)
skies g web-app Web --path clients/web   # a React web package, declared in Skies.toml and CI
skies g client --package clients/web     # its typed client, from the contract `dotnet build` writes
skies g feature <Name> --kind list|form --package clients/web   # a screen bound to that contract
```

After `g module`, write the module's `## Boundaries` and `## Design notes` in its ctx.md: the build fails until
they hold your own words. A scaffolded slice is mapped and in the contract from the start (so a client and a screen
can bind to it), but it answers its `<Slice>NotImplemented` business error until you write it: a spec's E2E against it
starts red, which is where a spec starts.

## Where things live

- `src/Golden.Api` — the backend. `Program.cs` is a thin index; `Modules/<Module>/` holds one bounded
  context: `<Module>Module.cs` (its wiring), `<Module>.ctx.md` (its why), entities at the root, `Slices/` (one
  operation per file). `BuildingBlocks/` holds shared value objects.
- `tests/Golden.Tests` — boots the real app and compiles the spec cases under `.specs/*/e2e`, nothing else.
- `.specs/<id>-<slug>/` — one feature: `spec.md`, `e2e/`, `receipt.json`.
- `Skies.toml` — the workspace topology, the root allowlist, and the spec runners. Put a new file under an existing
  folder; declare a new root entry only for a new top-level concern.

## Build and verify

```
skies doctor                 # the build with the doctor on, plus the root allowlist
dotnet test                  # every spec case
skies proof run <id>         # one spec's E2E, once, per failure mode; writes nothing
skies proof impact <paths>   # the specs a change reaches, before you make it
skies proof record <id>      # prove red and green, write the receipt
```

Nothing runs automatically. If the doctor is red, fix the code; never suppress a rule. Write comments for the
reader in domain terms, never to cite or appease a rule.

## Boundaries

- The framework ships the skeleton, the enforcement, and shared runtime mechanisms (auth lives in versioned Skies
  packages); this app brings its own libraries, vendor integrations, product policy, and business logic in plain
  code. No source generation of behavior, no base classes, no reflection-based discovery.
- The framework arrives only as versioned packages. A generic need (a rule, a primitive another Skies app would
  want) lands in Skies first and arrives here as a version bump; app-specific code stays here.
- Git: stage specific files, one commit per concern, lowercase imperative messages; a spec, its E2E, the code,
  and the receipt land together.

## The conventions

The full conventions ship in the Skies plugin's `docs/`: `CONVENTIONS.md` (backend, specs and proofs, the rule
catalog), `AUTH.md` (the auth package and what the app owns), `FRONTEND-CONVENTIONS.md` (a React web client), and
`FLUTTER-CONVENTIONS.md` (a Flutter client). Ground every convention fact there, never memory; read the frontend
docs when you add a client.
