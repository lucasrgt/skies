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
5. Tiers follow what a rule guards, decided after the calibration: an architecture rule is an error, a taste is a
   warning. `SKY0007` (the 500-line file ceiling) and `SKY0019` (an unused error-code constant) become warnings;
   `@skiesjs/eslint-plugin`'s `recommended` raises every SKYFE architecture rule to error, leaving SKYFE028, 031, and
   032 as warnings; `SKYFL009` (device plugins in a ViewModel) is retired with its React twin. The findings above
   are unchanged; only how loudly they are reported moved.

## Audit follow-up (2026-09-24)

An independent audit read the rules rather than their output and found precision gaps the first calibration could
not see, because the applications did not exercise them; twin rules that meant different things on the web and in
Flutter; three web rules still "(planned)" while their Flutter twins ran; no Flutter escape hatch; and the security
floor documented at error while CA2100 shipped as a warning the doctor dropped. Each fix carries a regression test,
and the calibration was re-run read-only on fresh copies of the Hostpoint and Marombas repositories (the same
packages as above; React parse-only with every `skies/*` rule on).

| Rule | Change | Before | After | Class of what remains | Test |
|---|---|---|---|---|---|
| SKYFL029 | a `refresh` call is a rotation only on the generated client or a session/auth receiver; Riverpod's `ref.refresh(provider)` is not | 0 | 0 | — | `twin_tests.rs` |
| SKYFE016 / SKYFL016 | a session key is a whole word (`token`, `jwt`, `session`, `auth`), so `authorName` is not; both twins share the predicate and the setter set | 1 / 0 | 1 / 0 | true: a legacy copy imports `setSession` | `viewmodel.test.cjs`, `twin_tests.rs` |
| SKYFE017 / SKYFL017 | one meaning: a redirect decided on an auth boolean (web: `<Navigate>` or `return`/`throw redirect(…)`; Flutter: a GoRouter `redirect:` or an `if` in a route/guard file), no longer any `isAuthenticated` read | 0 / 0 | 0 / 0 | — | same |
| SKYFL028 | only a ViewModel `onSuccess` whose whole body refetches, like SKYFE028 | 0 | 0 | — | `twin_tests.rs` |
| SKYFL032 | also accepts `forceErrorText` and a decoration `errorText`; a draft widening it to every `TextFormField` found 2 false positives (optional note fields with no validator have no error to lose), so it stays on the `lib/ui/` field primitive | 0 | 0 | — | `twin_tests.rs` |
| SKYFL007 | a purely local ViewModel (no client call, no `Future`/`Stream`) is not server-backed | 0 | 0 | — | `twin_tests.rs` |
| SKYFE004 | new: a ViewModel renders no JSX, imports no `react-dom` | — | 0 | — | `viewmodel.test.cjs` |
| SKYFE007 | new: a ViewModel exposing a query's `data` exposes its states. The first draft (per read, `AsyncState` only) found 84; 83 projected `isPending`/`isError` by hand or read a query their screen already loads, so explicit flags count and the rule asks once per ViewModel, as SKYFL007 does | — | 5 | true: form ViewModels whose secondary read (suggestions, a subscription flag, `me`) fails silently to a default | `viewmodel.test.cjs` |
| SKYFE023 / SKYFL023 | new on the web, warning in both. The draft matched `todo` in a Portuguese comment ("em todo o app"): markers are the uppercase ones in both twins | — / 0 | 6 / 0 | true: `@ts-expect-error`/`@ts-ignore` in a legacy design system and one View | same |
| SKY0004 | located on the ctx (the section's heading) or the `[Module]` class, never an arbitrary slice | 0 / 0 | 0 / 0 | — | `ModuleContextAnalyzerTests` |
| SKY0005 | a code citation needs a lowercase letter (`POST`, `JWT`, `SKY0005` are prose); spec citations are exactly `<spec>#FM-<n>`, and spec lines follow the proof engine's grammar (`cli/src/proof/grammar.rs`, pinned regex for regex by `catalog_tests.rs`); look-alikes are reported with the grammar | 0 / 0 | 0 / 0 | — | `ContextCitationGrammarTests` |
| SKY0006 | only a `*Repository`/`*UnitOfWork` that holds a `DbContext`/`DbSet` (and its interfaces); a vendor `GitHubRepository` is not | 0 / 0 | 0 / 0 | — | `NoRepositoryAnalyzerTests` |
| CA2100 | error, as the floor documents; the doctor reports every floor rule at any severity | 0 / 0 | 0 / 0 | — | `legs.rs` |

Unchanged elsewhere: SKY0029 1561 / 914, SKY0027 10 / 4 (warning), SKYFL036 387, SKYFL038 4, and every other React
count in the first table. Every rule the audit named was already quiet on these applications: its false positives
live in idioms they do not use (Riverpod, an `author` field, a vendor repository), which is why they are pinned by
tests rather than by counts.

Decisions, continuing the list above:

6. A shared number is one rule. `SKYFE0nn` and `SKYFL0nn` (up to 036) state the same intent at the same tier, in
   each ecosystem's spelling; `cli/src/doctor/catalog_tests.rs` pins both catalogs to the code that runs them and
   to each other, and no catalog lists a planned rule.
7. Tiers, reviewed rule by rule: an error guards architecture or security, a warning is a taste or a heuristic
   that cannot see everything. SKYFE023/SKYFL023 join SKY0007, SKY0019, SKY0026–0028, SKYFE/SKYFL028, 031, 032, and
   SKYFL039/040 as warnings; every other rule stays an error.
8. Every ecosystem has one narrow, visible hatch (`CONVENTIONS.md`, Suppression): `#pragma`/`[SuppressMessage]`,
   `eslint-disable-next-line … -- <reason>`, and the new `// skies-ignore: SKYFLnnn <reason>`, which requires the
   reason and is listed by `skies doctor`. No suppression was needed to reach the counts above.
