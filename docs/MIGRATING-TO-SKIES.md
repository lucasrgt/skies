# Migrating to Skies 5

Skies 5 keeps the runtime conventions (slices, modules, entities, value objects, MVVM, error codes, ctx.md) and
removes the verification apparatus around them. Read [the decision](decisions/skies-5-evidence-over-apparatus.md)
for the reasons.

## What changes

| Skies 4 | Skies 5 |
|---|---|
| `skies-framework-cli` dotnet tool | the `skies` binary (`npm install -g @skiesjs/cli` or `cargo install skies-cli`) |
| `skies gate`, `skies check`, lefthook hooks | nothing runs automatically; `skies doctor` and `skies proof` on demand |
| `<Module>.spec.toml`, `[AVP(...)]`, Assay.Net, avp-assay | `.specs/<id>/spec.md` with failure modes + `e2e/` + `receipt.json` |
| `[Journey]`, `[Unit]`, `[Integration]`, `[E2E]` | plain `[Fact]`; spec E2E use `DisplayName = "FM-n: …"` |
| `@verify`, `@avp`, `@e2e`, `flows.json`, backend ledger | removed |
| SKY0003, 0008, 0010, 0011, 0020, 0030–0033 | removed |
| SKYFE005, 006, 008, 012, 024–026, 033–035 and Flutter equivalents | removed |
| design tokens, `.design/`, design scaffolds | removed; styling is the app's |
| `csm.toml`, `.skies/`, `skies context`, `skies nya/wtw/rtw/nwc` | removed from Skies; the tools remain available on their own |
| `VERIFICATION.md`, `VERIFICATION.json` | removed |
| `Skies.toml` `[framework]`, `core`, `library`, `website` keys | `[products.*] backend` + `frontend` (one path or a list), plus `[runners.*]` |
| framework-sync, parity manifests | removed; all packages share one version |
| Node.js SDK (`@skiesjs/core`, `express`, …) | discontinued; stay on the last 4.x release |

## Upgrade an existing repository

1. Install the binary and run the migration from the repository root:

   ```bash
   npm install -g @skiesjs/cli
   skies migrate 5 --dry-run   # review
   skies migrate 5
   ```

   It removes the proof annotations and tags, `flows.json`, `*.spec.toml`, `VERIFICATION.*`, `csm.toml`,
   `.skies/csm`, the foundations block in `AGENTS.md`/`CLAUDE.md`, and the Skies hooks in `lefthook.yml`; rewrites
   `Skies.toml` to the 5.x schema; drops the `skies-framework-cli` dotnet tool; and adds the `.specs/` compile
   include to the test project.
2. Bump every `Skies.Framework.*`, `@skiesjs/*`, and `skies_flutter` reference to `5.0.0` and refresh lockfiles.
3. Remove `Assay.Net` / `avp-assay` / `assay-design` references if the migration reported any it could not edit.
4. Declare a runner in `Skies.toml` for each E2E engine you use (see
   [MONOREPO-ARCHITECTURE.md](MONOREPO-ARCHITECTURE.md)).
5. Run `skies doctor`, `dotnet test`, and your frontend tests.

Existing tests keep running as ordinary tests. They are not converted into specs. New features start with
`skies spec new`. For a critical area, write a spec after the fact and record it with a `red.patch` that removes the
behavior, so the receipt still shows every failure mode failing before and passing after.
