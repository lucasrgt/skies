# Skies — CLI reference

Generated from `skies <command> --help` by `cli/tests/plugin_docs.rs`; resync with
`tools/sync-plugin-docs.sh` instead of editing.

## skies

```text
Scaffold, check, and prove Skies applications.

Usage: skies <COMMAND>

Commands:
  new      Create a new Skies application in ./<Name>
  g        Generate code that follows the Skies conventions
  i18n     Assemble per-feature i18n catalogs into the package's locale files
  doctor   Run the architecture doctors: the declared repository root (SKYWS*), dotnet build (SKY* and the CA* security floor), eslint (SKYFE*), and the Flutter rules (SKYFL*). Errors fail the run; warnings are reported. A Flutter finding silenced by `// skies-ignore: SKYFLnnn <reason>` is listed with its reason, never dropped
  spec     Work with feature specs under .specs/
  proof    Record and check the evidence that a spec's failure modes are handled
  migrate  Migrate an application to a new Skies major version
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

## skies new

```text
Create a new Skies application in ./<Name>

Usage: skies new <NAME>

Arguments:
  <NAME>  The application name; it becomes the solution and root namespace

Options:
  -h, --help  Print help
```

## skies g

```text
Generate code that follows the Skies conventions

Usage: skies g <COMMAND>

Commands:
  module       A module: <Name>Module.cs and a ctx.md skeleton for you to write, wired into the module registry
  slice        A slice inside a module, mapped under the module's route group
  entity       A rich [Entity] with an EnsureValid invariant funnel
  vo           An always-valid [ValueObject] in BuildingBlocks
  crud         List/lookup/create/update/delete slices and a view record for an [Entity], its DbSet, and its Open/Update
  hub          A SignalR hub for real-time fan-out
  auth         The auth module: register, login, refresh, logout, me, sessions
  auth:otp     Phone verification by SMS code
  auth:oauth   Google sign-up and sign-in
  auth:email   Email verification and password reset
  feature      A frontend feature (ViewModel + View + i18n) in a React web or Flutter package
  client       The typed API client for a React web or Flutter package, from the backend's OpenAPI contract
  flutter-app  A Flutter application package wired to the Skies spine
  help         Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

## skies g module

```text
A module: <Name>Module.cs and a ctx.md skeleton for you to write, wired into the module registry

Usage: skies g module [OPTIONS] <NAME>

Arguments:
  <NAME>

Options:
      --project <DIR>  The .NET API project (its directory or .csproj) to write into. Defaults to the current directory's project, else the backend Skies.toml declares (with several, the one holding the module)
  -h, --help           Print help
```

## skies g slice

```text
A slice inside a module, mapped under the module's route group

Usage: skies g slice [OPTIONS] <MODULE> <NAME>

Arguments:
  <MODULE>
  <NAME>

Options:
      --project <DIR>  The .NET API project (its directory or .csproj) to write into. Defaults to the current directory's project, else the backend Skies.toml declares (with several, the one holding the module)
  -h, --help           Print help
```

## skies g entity

```text
A rich [Entity] with an EnsureValid invariant funnel

Usage: skies g entity [OPTIONS] <MODULE> <NAME>

Arguments:
  <MODULE>
  <NAME>

Options:
      --project <DIR>  The .NET API project (its directory or .csproj) to write into. Defaults to the current directory's project, else the backend Skies.toml declares (with several, the one holding the module)
  -h, --help           Print help
```

## skies g vo

```text
An always-valid [ValueObject] in BuildingBlocks

Usage: skies g vo [OPTIONS] <NAME>

Arguments:
  <NAME>

Options:
      --project <DIR>  The .NET API project (its directory or .csproj) to write into. Defaults to the current directory's project, else the backend Skies.toml declares (with several, the one holding the module)
  -h, --help           Print help
```

## skies g crud

```text
List/lookup/create/update/delete slices and a view record for an [Entity], its DbSet, and its Open/Update

Usage: skies g crud [OPTIONS] <MODULE> <ENTITY>

Arguments:
  <MODULE>
  <ENTITY>

Options:
      --project <DIR>  The .NET API project (its directory or .csproj) to write into. Defaults to the current directory's project, else the backend Skies.toml declares (with several, the one holding the module)
  -h, --help           Print help
```

## skies g hub

```text
A SignalR hub for real-time fan-out

Usage: skies g hub [OPTIONS] <MODULE> <NAME>

Arguments:
  <MODULE>
  <NAME>

Options:
      --project <DIR>  The .NET API project (its directory or .csproj) to write into. Defaults to the current directory's project, else the backend Skies.toml declares (with several, the one holding the module)
  -h, --help           Print help
```

## skies g auth

```text
The auth module: register, login, refresh, logout, me, sessions

Usage: skies g auth [OPTIONS]

Options:
      --skip-tenancy   Leave out multi-tenant scoping (the Tenancy/ files and the request tenant)
      --skip-cookies   Leave out web-cookie refresh delivery; the refresh token travels in the response body only
      --project <DIR>  The .NET API project (its directory or .csproj) to write into. Defaults to the current directory's project, else the backend Skies.toml declares (with several, the one holding the module)
  -h, --help           Print help
```

## skies g auth:otp

```text
Phone verification by SMS code

Usage: skies g auth:otp [OPTIONS]

Options:
      --project <DIR>  The .NET API project (its directory or .csproj) to write into. Defaults to the current directory's project, else the backend Skies.toml declares (with several, the one holding the module)
  -h, --help           Print help
```

## skies g auth:oauth

```text
Google sign-up and sign-in

Usage: skies g auth:oauth [OPTIONS]

Options:
      --project <DIR>  The .NET API project (its directory or .csproj) to write into. Defaults to the current directory's project, else the backend Skies.toml declares (with several, the one holding the module)
  -h, --help           Print help
```

## skies g auth:email

```text
Email verification and password reset

Usage: skies g auth:email [OPTIONS]

Options:
      --project <DIR>  The .NET API project (its directory or .csproj) to write into. Defaults to the current directory's project, else the backend Skies.toml declares (with several, the one holding the module)
  -h, --help           Print help
```

## skies g feature

```text
A frontend feature (ViewModel + View + i18n) in a React web or Flutter package

Usage: skies g feature [OPTIONS] <NAME>

Arguments:
  <NAME>

Options:
      --kind <KIND>        `list`: a read screen over the `List<Name>` query. `form`: a command screen that submits the `<Name>` mutation, with validation, pending, error, and success states [default: list] [possible values: list, form]
      --package <PACKAGE>  The frontend package directory (defaults to the current directory)
  -h, --help               Print help
```

## skies g client

```text
The typed API client for a React web or Flutter package, from the backend's OpenAPI contract

Usage: skies g client [OPTIONS]

Options:
      --package <PACKAGE>  The frontend package directory (defaults to the current directory)
      --input <INPUT>      Flutter: the OpenAPI document (defaults to the backend contract `Skies.toml` points at)
      --output <OUTPUT>    Flutter: the generated package directory (defaults to packages/<name>)
      --name <NAME>        Flutter: the generated Dart package name (defaults to <backend>_api)
      --version <VERSION>  Flutter: the generated package's pub version (defaults to 0.1.0)
  -h, --help               Print help
```

## skies g flutter-app

```text
A Flutter application package wired to the Skies spine

Usage: skies g flutter-app [OPTIONS] <NAME>

Arguments:
  <NAME>

Options:
      --path <PATH>  Where to create the package (defaults to ./<name>)
  -h, --help         Print help
```

## skies i18n

```text
Assemble per-feature i18n catalogs into the package's locale files

Usage: skies i18n [OPTIONS]

Options:
      --package <PACKAGE>  The frontend package directory (defaults to the current directory)
  -h, --help               Print help
```

## skies doctor

```text
Run the architecture doctors: the declared repository root (SKYWS*), dotnet build (SKY* and the CA* security floor), eslint (SKYFE*), and the Flutter rules (SKYFL*). Errors fail the run; warnings are reported. A Flutter finding silenced by `// skies-ignore: SKYFLnnn <reason>` is listed with its reason, never dropped

Usage: skies doctor [OPTIONS] [BUILD_ARGS]...

Arguments:
  [BUILD_ARGS]...  Extra arguments forwarded to `dotnet build`

Options:
      --package <PACKAGE>  Check only this package directory (a Flutter or React package, or a .NET project or its folder), so a package's own `lint` script can call the doctor (the root is not checked)
  -h, --help               Print help
```

## skies spec

```text
Work with feature specs under .specs/

Usage: skies spec <COMMAND>

Commands:
  new   Create .specs/<id>-<slug>/ with a spec.md to fill in
  help  Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

## skies spec new

```text
Create .specs/<id>-<slug>/ with a spec.md to fill in

Usage: skies spec new [OPTIONS] <SLUG>

Arguments:
  <SLUG>

Options:
      --runner <RUNNER>  The runner from Skies.toml that executes this spec's e2e/ folder
  -h, --help             Print help
```

## skies proof

```text
Record and check the evidence that a spec's failure modes are handled

Usage: skies proof <COMMAND>

Commands:
  run     Run the spec's E2E once on the working tree and print each failure mode's pass or fail, with what the failing cases reported. Writes only the spec's local evidence/raw/ (the report, run.log, and what the cases saved under $SKIES_EVIDENCE/raw/); never a receipt or committed evidence. Exits 1 unless every mode passes, and for a spec.md that lists no `- FM-<n>` line
  record  Run the spec's E2E on the red revision (every failure mode must fail) and on the working tree (every one must pass), then write receipt.json. Proves red->green once; CI keeps green passing afterwards. Cases that never ran on red count as failing only when the spec's own e2e files are why (a compile error or unresolved import in them); any other red failure (restore, missing tool, runner command) exits 2 with no receipt
  impact  Show which specs a change reaches: the specs cited by the ctx.md of every module the paths sit in (`**/Modules/<M>/` -> `<M>.ctx.md`), and the specs whose `touches:` globs match them, with their failure modes. With no paths, uses the files changed on this branch
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

## skies proof run

```text
Run the spec's E2E once on the working tree and print each failure mode's pass or fail, with what the failing cases reported. Writes only the spec's local evidence/raw/ (the report, run.log, and what the cases saved under $SKIES_EVIDENCE/raw/); never a receipt or committed evidence. Exits 1 unless every mode passes, and for a spec.md that lists no `- FM-<n>` line

Usage: skies proof run <SPEC>

Arguments:
  <SPEC>  The spec id or folder name

Options:
  -h, --help  Print help
```

## skies proof record

```text
Run the spec's E2E on the red revision (every failure mode must fail) and on the working tree (every one must pass), then write receipt.json. Proves red->green once; CI keeps green passing afterwards. Cases that never ran on red count as failing only when the spec's own e2e files are why (a compile error or unresolved import in them); any other red failure (restore, missing tool, runner command) exits 2 with no receipt

Usage: skies proof record [OPTIONS] <SPEC>

Arguments:
  <SPEC>  The spec id or folder name

Options:
      --red <RED>              The revision the failure modes must fail on. Defaults to HEAD plus the spec's red.patch when it has one, else the merge-base of HEAD with `[workspace] default_branch` from Skies.toml, origin/HEAD, main, or master, whichever exists first. The choice is printed
      --red-patch <RED_PATCH>  A patch applied to HEAD for red, for specs written after the code; kept as the spec's red.patch
  -h, --help                   Print help
```

## skies proof impact

```text
Show which specs a change reaches: the specs cited by the ctx.md of every module the paths sit in (`**/Modules/<M>/` -> `<M>.ctx.md`), and the specs whose `touches:` globs match them, with their failure modes. With no paths, uses the files changed on this branch

Usage: skies proof impact [PATHS]...

Arguments:
  [PATHS]...  Files or directories to look up, relative to the current directory

Options:
  -h, --help  Print help
```

## skies migrate

```text
Migrate an application to a new Skies major version

Usage: skies migrate [OPTIONS] <VERSION>

Arguments:
  <VERSION>  The target major version. Only 5 is supported

Options:
      --dry-run  Show what would change without writing
  -h, --help     Print help
```
