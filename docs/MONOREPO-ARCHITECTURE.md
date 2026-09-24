# Monorepo architecture

Skies treats a monorepo as explicit product topology plus independent, ordinary .NET, npm, and Dart packages.
`Skies.toml` names what exists and how spec E2E runs; it does not define tasks, dependencies, or generators. Build
behavior stays visible in project files, package scripts, and runner configuration.

## The workspace manifest

```toml
[workspace]
name = "hostpoint"
default_branch = "develop"
root = [".github/", ".specs/", "clients/", "docs/", "src/", "tests/", ".gitignore", "AGENTS.md", "README.md", "*.slnx"]

[products.marketplace]
backend = "src/Hostpoint.Api"
frontend = ["clients/hostpoint-web", "clients/hostpoint-mobile"]

[products.operator]
backend = "src/Hostpoint.Api"
frontend = "clients/hostpoint-os"

[runners.api]
build = "dotnet build tests/Hostpoint.Tests"
command = "dotnet test tests/Hostpoint.Tests --no-build --filter FullyQualifiedName~Specs.S{id}. --logger 'trx;LogFileName={report}'"

[runners.web]
setup = "docker compose up -d db"
command = "PLAYWRIGHT_JUNIT_OUTPUT_NAME={report} npx playwright test {dir} --reporter=junit"
```

Three sections exist, and unknown keys fail to parse:

- `[workspace]` names the repository. Optional `default_branch` names the branch features fork from; `skies proof
  record` takes red as the merge-base of HEAD with it (after `--red` and a spec's `red.patch`), and `skies proof
  impact` diffs from there. Without it, `origin/HEAD` (else `main`, else `master`) is used. Set it when work happens
  on a long-lived branch other than the remote's default.
  Required `root` lists everything allowed at the repository root (see [The root allowlist](#the-root-allowlist)).
- `[products.*]` lists each product's `backend` (a .NET application root), optional `tests` (the .NET project that
  compiles the spec E2E; `skies doctor` builds it instead of the backend, so `SKY0029` sees stray tests), and
  `frontend` packages (one path or a list; React has `package.json`, Flutter `pubspec.yaml`). A backend or package
  may appear in several products. `skies doctor` builds every backend and checks every frontend package listed here.
- `[runners.*]` are the commands that run one spec's E2E. Placeholders: `{id}` (the spec id), `{spec}` (its folder
  name), `{dir}` (its `e2e/` folder), `{report}` (where the JUnit or TRX report goes), and `{evidence}` (where a case
  saves artifacts to commit; also `SKIES_EVIDENCE`, with the spec folder name in `SKIES_SPEC`). Environment a tool
  needs goes in the command itself (`NAME={report} npx …`). Every command runs through `sh -c` (`cmd /C` on
  Windows), so shell syntax applies: quote an argument that holds a `;`, as the TRX logger's
  `'trx;LogFileName={report}'` does, or the shell ends the command there and no report is written. Keys, and unknown
  ones fail to parse:
  - `command` (required) runs from the checkout's project root; its exit code decides nothing, the report does.
  - `setup` runs first in each checkout (start a database, install packages). Red's checkout is a fresh git worktree
    inside the repository (`.skies-red/` at its top, excluded locally), so configuration found by walking up from
    the project (`NuGet.config`, `.npmrc`, `global.json`, tool manifests) applies to it as to the working tree, but
    ignored folders such as `node_modules/` are not there.
  - `build` runs once per checkout after `setup`, so `command` can skip compiling (`dotnet test --no-build`). A build
    that fails on red counts as `did-not-build` only when its errors sit in the spec's own `e2e/`; any other failure
    stops `skies proof record` without a receipt.

## The root allowlist

Repositories accumulate junk at the root: logs, screenshots, stray notes, a folder someone needed once. An agent adds
to it whenever it has nowhere better to put a file, and nobody decides to remove it. `[workspace] root` makes every
root entry a decision the manifest records: a new one is a one-line diff a reviewer sees, and anything else is a
`skies doctor` finding.

- Each entry is a glob ([globset](https://docs.rs/globset)) matched against one root entry's **name**, never a path:
  `src/`, `*.slnx`, `README.md`. `src/Api/` is rejected.
- **A trailing `/` matches a directory only; no trailing slash matches a file only.** `docs/` never allows a file
  named `docs`, and `README.md` never allows a folder; the finding says so when the other kind is declared.
- Matching is case-sensitive, and `*` matches dotfiles too. A name with glob characters is declared with them
  escaped as one-character classes (`[[]draft[]].md`); `skies migrate 5` and the doctor's suggestion do it for you.
- `.git` and `Skies.toml` are always allowed and never listed.
- Only what git would see counts: `.gitignore`d entries (build output, `node_modules/`, IDE state) are never flagged,
  and neither is a directory holding nothing git would see (an ignored agent-worktrees folder, an empty folder). The
  check reads one directory level (plus each root folder until its first visible file), spawns no process, and takes
  milliseconds.

The doctor's workspace leg reports `SKYWS001` (error) for each undeclared root entry ("move it under a declared
folder, delete it, or declare it") and `SKYWS002` (warning) for a declared entry that matches nothing on disk (a
stale line). A manifest without `root` gets one `SKYWS001` whose message carries today's root as a list ready to
paste. `skies new` writes the template's list, which declares exactly what a new app holds (its `README.md` included),
so a fresh app is clean of both findings; the usual later arrivals (`LICENSE`, `.env.example`, `docker-compose.yml`,
`global.json`, `nuget.config`) are named in its comment and declared with a line each when they arrive, and
`skies g web-app` declares the folder it creates. `skies migrate 5` declares the current root entries, so a migrated
app stays doctor-clean, and asks the owner to delete the junk and trim the list. `skies doctor --package` skips the
leg.

## Package ownership

Code stays in normal packages and remains stranger-maintainable:

```text
src/Hostpoint.Api/              ASP.NET application
clients/hostpoint-web/          React marketplace web app: features, generated client, i18n, ui/
clients/hostpoint-mobile/       Flutter marketplace app (phones; it could serve the web too)
clients/hostpoint-os/           React operator console
.specs/                         one folder per feature: spec.md, e2e/, receipt.json
```

React is the web body and Flutter the mobile one (and a supported web body), so a feature that exists on both
surfaces has two implementations with the same guarantees; the backend contract and the specs are what they share.
Promote a shared package only when at least two products on the same stack actually consume it. Capability seams
(storage, push, camera) are plain interfaces and adapters owned by the application; the framework does not generate
UI behavior or hide a runtime behind base classes.

## Specs across the stack

A spec belongs to the feature, not to a package. A feature that spans the API and two surfaces has one folder under
`.specs/` whose `e2e/` drives whichever surface proves it best; its `runner` names the engine. When one spec needs
two engines, split it into two specs that reference each other in their text.

## Package-first framework updates

Framework rules and shared primitives land in this repository first, are released as versioned NuGet, npm, and pub
packages (all at the same version), and are then consumed by applications. An application never keeps a private
copy of a framework rule or frontend plugin, so every app runs the same rules at the same version.

The canonical backend and frontend conventions remain in [CONVENTIONS.md](CONVENTIONS.md),
[FRONTEND-CONVENTIONS.md](FRONTEND-CONVENTIONS.md), and [FLUTTER-CONVENTIONS.md](FLUTTER-CONVENTIONS.md).
