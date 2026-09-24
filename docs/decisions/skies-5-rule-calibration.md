# Decision: Skies 5 rule calibration on real applications

**Status:** accepted.
**Date:** 2026-09-24.

## Context

A rule that cries wolf teaches people to ignore every rule. Before the first 5.0 release, each doctor ran on real
code it had never seen, read only, on copies:

- **React** (`@skiesjs/eslint-plugin` recommended: every `skies/*` rule plus the jsx-a11y floor, parse-only): the
  Marombas web client (TanStack Router, migrated to 5.0), Hostpoint's `app-core`, `hostpoint-app-deprecated`
  (Expo Router), `hostpoint-municipios-deprecated`, `hostpoint-os-deprecated`, `web-ui-deprecated`, and `website`
  (Astro; `.astro` files are out of ESLint's reach), and a non-Skies Next.js app (`lucro/apps/web`).
- **.NET** (`Skies.Framework.Doctor` built from the working tree): the Hostpoint and Marombas backends and tests.
- **Flutter** (`skies doctor --package`): Hostpoint's `hosts`, `travelers`, `municipalities`, `os-flutter`,
  `partners`, `design-system`, `app-core/flutter`, and their generated `*_api` client packages.

Each finding was sampled and classified. "True" means the code breaks the rule's stated contract; legacy code is
expected to break the spec rules (`SKY0029`, `SKYFE036`, `SKYFL036`).

## React (SKYFE and the a11y floor)

| Rule | Before | Class | Action | After |
|---|---|---|---|---|
| SKYFE011 i18n-completeness | 55 (Marombas) | false: every top-level object was a "catalog", so error-code maps and key tables were compared with the locale object; the real layout (`{ "pt-BR": {…}, "en-US": {…} }`) was never compared | compare only locale-named catalogs: sibling objects named like locales, or the children of one locale-keyed object | 0 |
| SKYFE036 tests-live-in-specs | 319 files | 313 true, 6 false: Playwright fixture files (`base.extend`, `test.step`) read as tests; 63 real test files missed (cases importing the app's re-exported, extended `test`; `node:test`) | only declaring members count (`describe`, `each`, `skip`…, not `extend`/`use`/`step`/hooks); `node:test` is a runner; a test-named import from the app's own module counts when called with a title and a body | 376, all true; a grep cross-check finds no miss and no hit outside a test file |
| SKYFE020 no-hardcoded-base-url | 5 | false: every one was Playwright's `use.baseURL` | a tool's own config (`playwright`/`vite`/`vitest`/`cypress`… `.config.*`) is not the client | 0 |
| SKYFE002 data-door | 1 | false: a test driving a generated hook to prove the client's transport | tests and spec cases exercise the door; exempt | 0 |
| SKYFE014 no-hardcoded-copy | 87 | true: hardcoded pt-BR JSX text and copy props in Views; no literal `aria-label`/`alt` appeared in a View | none | 87 |
| SKYFE013 mutation-error-handled | 12 | true under the bare default, and 0 under each app's own `{ globalSurface: true }` (their `QueryClient` wires the SKYFE027 defaults) | none: the option exists for exactly this | 12 (0 as configured) |
| SKYFE031 submit-handles-invalid | 18 (warn) | true to the contract; several are single-screen forms, the shape the warn tier allows | none: stays a warning | 18 |
| SKYFE015 no-router-replace-in-effect | 0 | route files under `src/routes/` were never scanned | route files now include TanStack's `src/routes/**` and React Router's `routes/` | 1, true (a redirect on `state.done` inside `useEffect`) |
| SKYFE016 / 027 / 029 | 1 / 2 / 1 | true: a legacy app copy parked under `storybook/`, and a non-Skies app's bare `QueryClient` | none | same |
| SKYFE021 no-raw-html | 3 | true to the contract: JSON-LD and a theme `<style>` through `dangerouslySetInnerHTML`, unescaped; the `lib/html` seam is where the `<` escape belongs | none | 3 |
| SKYFE018 / 019 / 022 | 0 | the apps use strict `Route.useParams()`, `useGoBack`, and no URL-derived targets | precision fixes for idioms these apps do not use yet, pinned by tests: SKYFE018 accepts `throw notFound()` and `invariant(id)` (not `return null`) and sees a cast `useParams() as {…}`; SKYFE019 accepts TanStack's `useCanGoBack()`; SKYFE022 checks only the navigation target (a value forwarded in `search`, a lookup key `ROUTES[next]`, a conditional's test are not a redirect), reads `Route.useSearch()`, and covers `<Navigate to={…}>` | 0 |
| jsx-a11y/label-has-associated-control | 1 | false: label text three levels deep (`<label><input/><span><strong>{name}`) | `depth: 3` in recommended | 0 |
| jsx-a11y/heading-has-content | 1 | false: a shadcn-style `<h3 {...props} />` wrapper gets its children through the spread | none: an upstream limit; relax it for the wrapper file, as the floor's docs describe | 1 |
| jsx-a11y (autofocus, media captions, static/non-interactive handlers) | 14 | true: `autoFocus`, a `<video>` without a track, mouse-only modal backdrops | none | 14 |

Not fixable in a rule, and not a false positive: SKYFE014 flags a brand name in JSX text the same as copy; scope
stays Views only, so hardcoded copy in a route file (seen once) is not caught.

## .NET (SKY)

| Rule | Hostpoint | Marombas | Class | Action |
|---|---|---|---|---|
| SKY0029 tests live in specs | 1561 (358 files) | 914 (173 files) | exact: every hit is a `[Fact]`/`[Theory]` in a test file, and the attribute counts equal the hit counts per file | none |
| SKY0005 ctx freshness | 0 | 1 | false: `` `2026-31` `` (an ISO week example) read as a spec citation, because a slug could be all digits | a spec slug must hold a letter; `` `0009-2fa-login` `` still cites a spec, `` `2024-01-15` `` and `` `1-5` `` are prose |
| every other SKY rule | 0 | 0 | — | — |

Marombas' tests project does not reference `Skies.Framework.Doctor` (the API's reference is `PrivateAssets="all"`),
so as configured SKY0029 never runs there; the count above adds the reference. That is the app's setup, which
`CONVENTIONS.md` already covers.

## Flutter (SKYFL)

| Rule | Before | Class | Action | After |
|---|---|---|---|---|
| SKYFL036 tests live in specs | 387 files | exact: none in `lib/`, none missed | none | 387 |
| SKYFL001 view-purity | 32 | false: built_value DTOs named `*_view.dart` in generated `*_api` client packages | a package with the `.skies-generated-client` marker is skipped whole; generated sources are skipped by every rule, not only the a11y floor | 0 |
| SKYFL002 data-door | 4 | false: 3 integration-test harnesses seeding the backend, 1 `part of` file of a ViewModel | everything under `test/` and `integration_test/` is test code and meets only SKYFL036; a `part of` file takes its library's role | 0 |
| SKYFL018 route-param-guard | 2 | false: test-only `GoRoute` builders | same test-code fix | 0 |
| SKYFL019 safe-back | 7 | false: 4 `popOrGo` helpers guarded by `canPop`, 3 overlay closes; and every pop in an arrow body (`() => Navigator.of(context).pop()`) was missed, because the grammar folds the arrow into the receiver | the receiver is normalized; a file using `canPop`, a pop returning a result, a pop inside a `show*` builder, a pop in a file that `Navigator.push`es its own page, and a pop of an overlay's own route context are not a Back | 0 (the arrow-body pops now seen all close an overlay or a pushed page) |
| SKYFL038 image-semantics | 4 | true: images with no label and no exclusion | none | 4 |

Also newly in reach: 13 `part of '*_view.dart'` files now meet the View rules (no findings today).

## Decision

1. Fix every false positive class found, each pinned by a regression test: `calibration.test.cjs` (ESLint),
   `ContextFreshnessAnalyzerTests` (SKY0005), `calibration_tests.rs` (SKYFL).
2. No rule is downgraded or removed. The noisiest classes (SKYFE011, SKYFE036 fixtures, SKYFL001 on generated code,
   SKYFL019 on overlays) had a structural cause and a precise fix; what remains is true to each rule's contract.
   SKYFE031 and SKYFE028 stay warnings, as before.
3. Route files are `app/`, TanStack Router's `src/routes/**`, and React Router's `routes/`; the generated
   `routeTree.gen.ts` and tests are not.
4. Re-run this calibration before each major release on the same applications.
