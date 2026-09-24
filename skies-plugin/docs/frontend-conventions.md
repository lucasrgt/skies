# Skies — Frontend conventions

React for the web (react-dom, TanStack Router or React Router, typed routes on). Mobile is Flutter, which can
serve the web too when a product wants one codebase for every surface; its conventions mirror these (SKYFL###).
A package shared by two web products is promoted only when ≥2 products consume it. `Skies.toml` lists the frontend
packages `skies doctor` checks.

## MVVM triple (one screen = one feature folder)

- View (`*.view.tsx`): pure render, exactly one ViewModel, mock-free by construction.
- ViewModel (`*.viewModel.ts`): plain hook `use<Name>Model(params) → { state, ...commands }`,
  render-agnostic (browser capabilities arrive as injected ports: storage, clipboard, geolocation).
- i18n per feature: `<feat>.i18n.ts` exports per-locale objects; assembled by `skies i18n` into
  `src/i18n/resources.generated.ts`; shared copy in the `common` namespace; Views read
  `useTranslation("<feat>")`.

## Data wiring

orval (stock, react-query target, wrapped not forked) generates `client.gen/`: one typed hook
per slice, named after the slice (backed by `.WithName(...)` = operationId). The mutator
(`lib/skies-client.ts`) injects auth, sends `X-Client: web` (the cookie session), maps errors. The audience
filter keeps only app endpoints (`WithEndpointKind(EndpointKind.Asset|Webhook|Internal)` are excluded). ViewModels compose
generated hooks directly — never wrap in custom repositories. Completeness is the compiler:
an invented endpoint doesn't exist as an export. Regenerate with `skies g client`.

## Async states

ViewModels expose `loading`/`error`/`empty` explicitly (`toAsyncState`, `combineAsyncStates`
from the @skiesjs/react spine). Views route all three through `<Resource>` (with retry).

## Mutations & feedback

QueryClient carries defaults: mutationCache onSuccess invalidates active queries + posts a
success note through the `lib/feedback` seam; onError routes to feedback unconditionally.
`meta: { silent: true }` skips the note (sign-in, reorder); `meta: { expectedFailure: true }`
for modeled failures. No per-call refetch rituals. Every mutate call still surfaces failure
somewhere visible.

## Session

One door: token writes via `lib/session` seam, paired with `me`-cache reset. The refresh token is an httpOnly
cookie; rotation is the seam's single-flight `bootstrapSession`, used by the client's 401 refresh+replay and by the
boot gate (`useSession`, navigation gated on `ready`). Never a second rotation path. Guards read tri-state
`SessionState`. Required params via `requiredParam()`; back via `safeBack`; redirects declarative (`<Navigate>`);
URL-sourced navigation through an allowlist.

## Forms

react-hook-form lives in the ViewModel; zod schema grounded in the contract (required +
documented patterns lifted from OpenAPI; closed enums via controlled pickers). Field anatomy:
`Field` wires label↔control; hint is replaced by error (`role="alert"`, danger color); mutation
errors get their own alert block above submit; submit Button carries `loading`. Big forms:
spine (VM + tab shell) + pure `panels/<X>Panel.view.tsx` binding the shared `control`.

## Styling and a11y

Styling, components, and tokens are the app's choice. A11y: jsx-a11y beside the SKYFE plugin.

## Specs

Features are proven by `.specs/<id>-<slug>/`: failure modes first, then cases (Playwright or Vitest for React,
`flutter_test` or `integration_test` for Flutter) titled `FM-n: …`, then `skies proof record`. Drive the real UI against the real API;
nothing in the ViewModel or View points at the spec. Every test lives in a spec (SKYFE036, SKYFL036): no
`*.test.tsx` beside the code, no package `test/` folder; an isolated unit gets its own spec. Flutter cases import
`package:<app>/...`, so the runner copies the spec's `e2e/` into the package's hidden `.skies_spec/` to run them.

## Comments

Default none. A comment earns its place saying what code can't: a non-obvious why, a gotcha,
an invariant. English only; user copy is i18n pt-BR.
