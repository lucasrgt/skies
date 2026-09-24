# Monorepo architecture

Skies treats a monorepo as explicit product topology plus independent, ordinary .NET, npm, and Dart packages.
`Skies.toml` names what exists and how spec E2E runs; it does not define tasks, dependencies, or generators. Build
behavior stays visible in project files, package scripts, and runner configuration.

## The workspace manifest

```toml
[workspace]
name = "hostpoint"

[products.marketplace]
backend = "src/Hostpoint.Api"
frontend = ["clients/app-core", "clients/hostpoint-app"]

[products.operator]
backend = "src/Hostpoint.Api"
frontend = "clients/hostpoint-os"

[runners.api]
command = "dotnet test tests/Hostpoint.Tests --filter FullyQualifiedName~Specs.S{id}. --logger trx;LogFilePath={report}"

[runners.web]
command = "npx playwright test {dir} --reporter=junit"
env = { PLAYWRIGHT_JUNIT_OUTPUT_NAME = "{report}" }
setup = "docker compose up -d db"
```

Three sections exist, and unknown keys fail to parse:

- `[workspace]` names the repository.
- `[products.*]` lists each product's `backend` (a .NET application root) and `frontend` packages (one path or a
  list; a React package has `package.json`, a Flutter package has `pubspec.yaml`). A backend or package may appear
  in several products. `skies doctor` builds every backend and checks every frontend package listed here.
- `[runners.*]` are the commands that run one spec's E2E. Placeholders: `{id}` (the spec id), `{spec}` (its folder
  name), `{dir}` (its `e2e/` folder), `{report}` (where the JUnit or TRX report goes), `{evidence}` (the spec's
  evidence folder for screenshots and logs). `setup` runs once before the first spec that uses the runner.

There is no verification setting, no gate mode, and no framework checkout reference.

## Package ownership

Code stays in normal packages and remains stranger-maintainable:

```text
src/Hostpoint.Api/              ASP.NET application
clients/app-core/               shared ViewModels, Views, generated client, i18n
clients/hostpoint-app/          executable marketplace surface
clients/hostpoint-os/           executable operator surface
.specs/                         one folder per feature: spec.md, e2e/, receipt.json
```

Promote a shared package only when at least two products actually consume it. Platform capability seams are plain
interfaces and adapters owned by the application; the framework does not generate UI behavior or hide a runtime
behind base classes.

## Specs across the stack

A spec belongs to the feature, not to a package. A feature that spans the API and two surfaces has one folder under
`.specs/` whose `e2e/` drives whichever surface proves it best; its `runner` names the engine. When one spec needs
two engines, split it into two specs that reference each other in their text.

## Package-first framework updates

Framework rules and shared primitives land in this repository first, are released as versioned NuGet, npm, and pub
packages (all at the same version), and are then consumed by applications. A pilot never keeps a private copy of a
framework rule or frontend plugin.

The canonical backend and frontend conventions remain in [CONVENTIONS.md](CONVENTIONS.md),
[FRONTEND-CONVENTIONS.md](FRONTEND-CONVENTIONS.md), and [FLUTTER-CONVENTIONS.md](FLUTTER-CONVENTIONS.md).
