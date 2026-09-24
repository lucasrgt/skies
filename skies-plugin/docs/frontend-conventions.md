# Skies — Frontend conventions

React Native (Expo) mobile + RN-web + Astro (public/SEO); admin OS in react-dom. Per-product
`core` (ViewModels + client + i18n + model, platform-agnostic) promoted to `shared/` only at
≥2 products. `shared/kernel` holds auth/client/spine/ports (no app↔os edge). `Skies.toml` lists
the frontend packages `skies doctor` checks.

## MVVM triple (one screen = one feature folder)

- View (`*.view.tsx`): pure render, exactly one ViewModel, mock-free by construction.
- ViewModel (`*.viewModel.ts`): plain hook `use<Name>Model(params) → { state, ...commands }`,
  render-agnostic AND platform-agnostic (no react-native/expo imports — capabilities arrive as
  injected ports: storage, navigator, push, file picker, linking, maps).
- i18n per feature: `<feat>.i18n.ts` exports per-locale objects; assembled in
  `src/i18n/resources.ts`; shared copy in the `common` namespace; Views read
  `useTranslation("<feat>")`.

## Data wiring

orval (stock, react-query target, wrapped not forked) generates `client.gen/`: one typed hook
per slice, named after the slice (backed by `.WithName(...)` = operationId). The mutator
(`skies-client.ts`) unwraps Result<T>, injects auth, maps errors. Audience filter keeps only
this frontend's endpoints (webhooks/internal/other audiences excluded). ViewModels compose
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

One door: token writes via `lib/session` seam, paired with `me`-cache reset. Rotation:
web = client seam's single-flight 401 refresh+replay; native = gated bootstrap (token exchanged
once at boot, navigation gated on `ready`). Never both. Guards read tri-state `SessionState`.
Required params via `requiredParam()`; back via `safeBack`; redirects declarative; URL-sourced
navigation through an allowlist.

## Forms

react-hook-form lives in the ViewModel; zod schema grounded in the contract (required +
documented patterns lifted from OpenAPI; closed enums via controlled pickers). Field anatomy:
`Field` wires label↔control; hint is replaced by error (`role="alert"`, danger color); mutation
errors get their own alert block above submit; submit Button carries `loading`. Big forms:
spine (VM + tab shell) + pure `panels/<X>Panel.view.tsx` binding the shared `control`.

## Styling and a11y

Styling, components, and tokens are the app's choice. A11y: jsx-a11y (web) / react-native-a11y (mobile).

## Specs

Features are proven by `.specs/<id>-<slug>/`: failure modes first, then black-box E2E (Playwright on web, Maestro
on native) titled `FM-n: …`, then `skies proof record`. Drive the real UI against the real API; nothing in the
ViewModel or View points at the spec.

## Comments

Default none. A comment earns its place saying what code can't: a non-obvious why, a gotcha,
an invariant. English only; user copy is i18n pt-BR.
