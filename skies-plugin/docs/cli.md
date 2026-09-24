# Skies — Markers & CLI

## Backend markers (pure attributes; inert if the doctor is removed)

- `[Slice]` — nested `Input`/`Output` records, `Handle(...) → Task<Result<Output>>`, `Map(IEndpointRouteBuilder)`, in order.
- `[Module]` — static class with `AddServices(IServiceCollection, IConfiguration)` + `Map(IEndpointRouteBuilder)`.
- `[ValueObject]` — immutable, no public ctor/setter, smart constructor returning `Result<T>`.
- `[Entity]` — private ctor, private setters, private `EnsureValid() → Result<T>` funnel.
- `[JsonConverter(typeof(ScalarJsonConverter<TVo, TPrim>))]` — a scalar VO crosses the wire as its primitive.
- `[Endpoint(...)]` — endpoint nature: default `App`; `Webhook`, `Internal`, or `Audience = "admin"`.

## Frontend markers (file conventions)

- `<Name>.view.tsx` / `<name>_view.dart` — pure render, exactly one ViewModel.
- `<Name>.viewModel.ts` / `<name>_view_model.dart` — render- and platform-agnostic.
- `<name>.i18n.ts` / ARB catalogs — per-feature copy.

## CLI (`skies`)

- `skies new <Name>` — a new app: Skies.toml, `src/<App>.Api`, a Health module, tests project, `.specs/`.
- `skies g module|slice|entity|vo|crud|hub <…>` — backend shapes, doctor-clean, no generated tests.
- `skies g auth [--skip-tenancy] [--skip-cookies]`, `g auth:otp|auth:oauth|auth:email` — auth blueprints, each with
  its own spec folder and E2E.
- `skies g client [--package <dir>]` — typed client (orval for React, dart-dio for Flutter).
- `skies g feature <Name> [--package <dir>]` — a ViewModel + View + i18n feature (React or Flutter).
- `skies g flutter-app <Name> [--path <dir>]` — wire a Flutter app to the spine.
- `skies i18n [--package <dir>]` — assemble per-feature catalogs.
- `skies doctor [build args]` — dotnet build (SKY*), eslint (SKYFE*), Flutter rules (SKYFL*), in parallel.
- `skies spec new <slug> [--runner <name>]` — create `.specs/<id>-<slug>/`.
- `skies proof record <id> [--red <rev>] [--red-patch <file>]` — red then green; writes `receipt.json`.
- `skies proof status` — which receipts are current or stale (hashes only).
- `skies proof verify <ids…> | --stale | --all` — rerun and refresh green evidence.
- `skies migrate 5 [--dry-run]` — move a 4.x app onto Skies 5.
