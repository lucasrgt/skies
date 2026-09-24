# Skies — Doctor rules (SKY* backend, SKYFE* frontend, SKYFL* Flutter)

Architecture only. Never suppress: a firing rule means the shape is wrong; fix the shape.

## Backend (Roslyn)

- SKY0001 slice conformance (static class, Input/Output, Handle→Task<Result<T>>, Map, ordered)
- SKY0002 endpoint thin (expression/method group, never statement block)
- SKY0004 every module has `<Module>.ctx.md` with non-empty `## Boundaries` + `## Design notes`
- SKY0005 ctx.md fresh: backticked citations resolve in source (not mtime)
- SKY0006 no IRepository / unit-of-work in slices
- SKY0007 file ≤ 500 LOC (Migrations/ exempt)
- SKY0009 write-ownership: module writes only its own entities (reads/joins free; Tests exempt)
- SKY0012 Map calls `.WithName("<SliceName>")` (operationId = frontend hook name)
- SKY0013 [ValueObject] always-valid (immutable, smart constructor Result<T>)
- SKY0014 [Entity] encapsulation (private ctor/setters, EnsureValid funnel)
- SKY0015 [Module] shape (AddServices + Map)
- SKY0016 every module registered in explicit AddModules/MapModules
- SKY0017 Program.cs is an index (AddSkies/AddPlatform/AddModules + Use/Map only)
- SKY0018 error code is a registry constant on *ErrorCodes (never literal)
- SKY0019 every *ErrorCodes constant is used (no orphans)
- SKY0021 unmarked domain types: DbSet<T> unmarked → [Entity]; complex member of [Entity] unmarked → [ValueObject]
- SKY0022 every endpoint declares authorization (.RequireAuthorization or .AllowAnonymous)
- SKY0023 injected ICurrentUser must be consulted
- SKY0024 raw SQL never absorbs runtime values as text (FromSql/ExecuteSql parameterized)
- SKY0025 held Result<T> checked before .Value/.Error
- SKY0026 every persisted write declares concurrency posture (warning tier)

Self-harness (framework dev only): SKYSELF001 ≤500 lines · SKYSELF002 no TODO/FIXME/HACK ·
CS1591 public members documented.

## Frontend (@skiesjs/eslint-plugin)

- SKYFE001 View purity (no data layer in *.view.tsx; type-only contract imports OK)
- SKYFE002 ViewModel data door (only VMs + lib/session + lib/guards import client.gen)
- SKYFE003 no mocks/MSW outside *.test.*
- SKYFE004 (planned) VM imports no JSX/react-dom
- SKYFE007 (planned) VM exposes loading/error/empty
- SKYFE009 VM platform-agnostic (no react-native/expo-*; ports injected)
- SKYFE010 Views route async states through <Resource> (no raw isPending/isError)
- SKYFE011 i18n parity: every locale declares the same flattened keys
- SKYFE013 every mutation surfaces failure (empty onError flagged)
- SKYFE014 no hardcoded user-facing copy in Views (t() only)
- SKYFE015 no imperative redirect in useEffect (declarative <Redirect/>)
- SKYFE016 session one-door: token writes via lib/session seam (+me-cache reset)
- SKYFE017 guards read tri-state SessionState, never raw boolean
- SKYFE018 required route params via requiredParam() union
- SKYFE019 no bare router.back()/history.back() (safeBack/useGoBack)
- SKYFE020 no hardcoded API base URL (env/relative/injected)
- SKYFE021 no dangerouslySetInnerHTML outside audited lib/html seam
- SKYFE022 no open redirect (URL-sourced navigation through in-app allowlist)
- SKYFE023 (planned) no orphan placeholders (// wire later, TODO, @ts-expect-error on data call)
- SKYFE027 QueryClient carries mutation defaults (invalidate + feedback; meta.silent/expectedFailure opt-outs)
- SKYFE028 (warn) no onSuccess refetch ritual (defaults already invalidate)
- SKYFE029 refresh one-door (only lib/skies-client, lib/session)
- SKYFE030 no cast on navigation targets
- SKYFE031 submit handles the invalid form path
- SKYFE032 Controller surfaces fieldState validation errors

## Flutter (native in `skies doctor`)

SKYFL### rules mirror the SKYFE numbers for the same concern; see `docs/FLUTTER-CONVENTIONS.md` in the
framework repository.
