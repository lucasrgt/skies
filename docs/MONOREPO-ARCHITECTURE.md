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
scope = ["src/"]
build = "dotnet build tests/Hostpoint.Tests"
command = "dotnet test tests/Hostpoint.Tests --no-build --filter FullyQualifiedName~Specs.S{id}. --logger trx;LogFilePath={report} --collect \"XPlat Code Coverage\" --results-directory {coverage}"

[runners.web]
scope = ["clients/hostpoint-web/"]
command = "npx playwright test {dir} --reporter=junit"
env = { PLAYWRIGHT_JUNIT_OUTPUT_NAME = "{report}" }
setup = "docker compose up -d db"
```

Three sections exist, and unknown keys fail to parse:

- `[workspace]` names the repository. Optional `default_branch` names the branch features fork from; `skies proof
  record` takes red as the merge-base of HEAD with it (after `--red` and a spec's `red.patch`), and `skies proof
  impact` diffs from there. Without it, the current branch's upstream (when it is another branch) and then
  `origin/HEAD` are used. Set it when work happens on a long-lived branch other than the remote's default.
  Required `root` lists everything allowed at the repository root (see [The root allowlist](#the-root-allowlist)).
- `[products.*]` lists each product's `backend` (a .NET application root), optional `tests` (the .NET project that
  compiles the spec E2E; `skies doctor` builds it instead of the backend, so `SKY0029` sees stray tests), and
  `frontend` packages (one path or a list; React has `package.json`, Flutter `pubspec.yaml`). A backend or package
  may appear in several products. `skies doctor` builds every backend and checks every frontend package listed here.
- `[runners.*]` are the commands that run one spec's E2E. Placeholders: `{id}` (the spec id), `{spec}` (its folder
  name), `{dir}` (its `e2e/` folder), `{report}` (where the JUnit or TRX report goes), `{evidence}` (the spec's
  evidence folder for screenshots and logs), `{coverage}` (where coverage goes). Keys:
  - `command` (required) runs from the checkout's project root; its exit code decides nothing, the report does.
  - `setup` runs once per checkout per invocation before the first spec that uses the runner (start a database).
  - `build` runs once per checkout per invocation after `setup`, so `command` can skip compiling
    (`dotnet test --no-build`); a failing build on red counts as `did-not-build`.
  - `scope` lists the paths (relative to the root) that belong to this runner's specs: only files under them count
    for the diff part of a footprint, `touches` matches, coverage, and ctx notes, so an API spec never pins a screen
    and a screen's spec never pins the backend. Without it, every file in the project counts.
  - `coverage` pins where the runner writes Cobertura or LCOV (a file or a folder, relative to the root) when the
    tool cannot take `{coverage}`; with coverage, a footprint is the lines green executed.
  - `report` pins the report path (relative to the root) when the tool cannot take `{report}`.
  - `env` adds variables (placeholders expand in values); `SKIES_EVIDENCE`, `SKIES_SPEC`, and `SKIES_COVERAGE` are
    always set.

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
paste. `skies new` writes the template's list; `skies migrate 5` declares the current root entries, so a migrated app
stays doctor-clean, and asks the owner to delete the junk and trim the list. `skies doctor --package` skips the leg.

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
