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
| `skies context`, `skies nya/wtw/rtw/nwc` | removed from Skies; the tools remain available on their own, and `csm.toml` + `.skies/csm` stay in the repository |
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

   A second `skies migrate 5 --dry-run` reports zero changes. The migration never deletes a test; it edits only the
   lines it must, keeps formatting and key order, and lists everything it could not decide under "Finish by hand".

   | Area | What `skies migrate 5` does |
   |---|---|
   | Proof ceremony | removes `[AVP]`, `[Journey]`, `[Unit]`, `[Integration]`, `[E2E]` (other attributes on the line stay), the `@verify`/`@avp`/`@e2e` doc tags, `e2e/flows.json`, `*.spec.toml`, `VERIFICATION.*`; keeps `csm.toml` and `.skies/csm` (the team's records) and says so |
   | Agent instructions and hooks | removes the `skies:foundations` block in `AGENTS.md`/`CLAUDE.md` and the hook commands in `lefthook.yml` that ran `skies check`/`gate`/`context`; reports prose that still tells agents to run the gate |
   | `Skies.toml` | rewrites it to the 5.x schema (`core`/`library`/`website` fold into `frontend`; `[framework]` goes) |
   | .NET | drops the `skies-framework-cli` dotnet tool (and the manifest when nothing else is in it); sets every `Skies`/`Skies.Framework*` `PackageReference`/`PackageVersion` to the binary's version; adds the `.specs/` compile include to the test project |
   | `package.json` | removes `@skiesjs/frontend-sdk` (and any alias of it) and `skies-flutter`; renames `skies-react`/`eslint-plugin-skies` to `@skiesjs/react`/`@skiesjs/eslint-plugin` (imports too); sets both to the binary's version |
   | npm scripts | in each `&&` chain, drops segments that run removed bins (`skyfe-*`, the `skies-flutter-*` checkers, `skies check`/`gate`/`context`, `dotnet tool run skies check`); rewrites `skies-flutter-doctor <dir>` to `skies doctor --package <dir>`, `skies-flutter-client …` to `skies g client --package . …` (same flags), and `skies-flutter-i18n` to `skies i18n`; deletes a script left empty along with its `pre`/`post` hooks and the `npm run` calls to it; lists every edited script per file |
   | ESLint | removes settings for `skies/*` rules the 5.x plugin no longer ships (`test-colocated`, `view-integration-test`, `design-tokens`, `ui-door`, `scale-only`, `semantic-colors`, `verify-has-avp-proof`, `no-disabled-tests`, `feature-has-e2e-flow`, …) from `eslint.config.*` and `.eslintrc*` |
   | Flutter | sets a hosted `skies_flutter` dependency to the binary's version (path and git dependencies stay) |
   | Test helpers | copies the removed Playwright fixtures, backend ledger, and Assay adapter (`@skiesjs/frontend-sdk/playwright*`, `…/product-verification`) and the Dio ledger (`skies_flutter_testing.dart`) into the app and points the imports at the copies |
   | Auth | never rewrites a generated Account module (the code is the app's, possibly changed); when a Skies 4 `Modules/Account/Slices/Refresh.cs` still carries its own rotation, notes that the mechanics now live in `Skies.Framework.Auth` (see CONVENTIONS.md, Auth): regenerate with `skies g auth` in a branch and compare |
   | CI | removes workflow steps that run the gate (only the gate lines of a `run: \|` block), and `dotnet tool restore` when the tool manifest was deleted |

2. Work through "Finish by hand". Typical items: run `npm install` and `flutter pub get` to refresh lockfiles, run
   `dotnet format --diagnostics IDE0005` for usings only the removed attributes needed, review the rewritten scripts,
   and decide about `Assay.Net` / `avp-assay` / `assay-design`, which are independent tools the app may keep.
3. Declare a runner in `Skies.toml` for each E2E engine you use (see
   [MONOREPO-ARCHITECTURE.md](MONOREPO-ARCHITECTURE.md)).
4. Run `skies doctor`, `dotnet test`, and your frontend tests. A package's `lint` script can call
   `skies doctor --package .` to run only its own leg.

Existing tests keep running as ordinary tests. They are not converted into specs. New features start with
`skies spec new`. For a critical area, write a spec after the fact and record it with a `red.patch` that removes the
behavior, so the receipt still shows every failure mode failing before and passing after.

## Runtime and generator changes

- `IExternalIdentity` and its synchronous `Verify` are removed. Implement `IExternalIdentityVerifier.VerifyAsync`
  instead; discovery/key retrieval is asynchronous. `ExternalUser` retains its shape.
- New auth scaffolds use the standard module registry and put provider selection in `Platform.AddPlatform`.
  Existing generated code remains app-owned: compare a fresh scaffold and merge the changes manually.
  Development providers refuse to start outside Development. Before deployment, configure a persistent AppDb,
  a private `Jwt:Secret`, and real email/SMS/OIDC providers in that platform setup.
- Value-object and entity rules reject accessible `init` accessors. Use get-only or private-init properties.
  Entities no longer need an unused `EnsureValid` method to satisfy the doctor; factories and mutations still
  own validation. A failed mutation must leave the entity unchanged.
- Session seams discard a refresh that predates sign-out or another sign-in. Flutter serializes secure-storage
  writes as well, so sign-out cannot leave a refresh credential saved by an older request.
