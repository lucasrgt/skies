# @skiesjs/eslint-plugin

The SKYFE architecture rules for Skies React web apps — the front-side parallel of the backend's Roslyn
analyzers (`Skies.Framework.Doctor`). They keep the MVVM seam honest: the View renders, the ViewModel is the only
data door, and routing, session and forms each go through one seam. Doctor-removable: delete the plugin and the app
still builds; you only lose the warnings.

## Rules

| Rule | Code | Polices |
|---|---|---|
| `view-purity` | SKYFE001 | A `*.view.tsx` renders only — no generated client / axios / react-query import (contract **types** are fine). |
| `data-door` | SKYFE002 | The generated client is imported only by a `*.viewModel.ts` or the auth/routing infra (`lib/session`, `lib/guards`); re-exporting it elsewhere is flagged too. |
| `no-mock` | SKYFE003 | No mock/fixture/MSW import in production code (only under `*.test.*`). |
| `state-completeness` | SKYFE010 | A View routes loading/error/empty through `<Resource>` — no raw `isPending`/`isError`/… |
| `i18n-completeness` | SKYFE011 | Every locale in a `*.i18n.ts` declares the same (flattened) keys. |
| `mutation-error-handled` | SKYFE013 | A ViewModel mutation surfaces its failure (`onError`, a read `.isError`, or a caught/propagated `mutateAsync`); an empty `onError` is flagged. `{ globalSurface: true }` trusts the QueryClient defaults. |
| `no-hardcoded-copy` | SKYFE014 | No hardcoded user-facing text in a View — JSX text and copy props go through `t()`. |
| `no-router-replace-in-effect` | SKYFE015 | Redirect declaratively (`<Navigate>`), never `router.navigate`/`navigate()` inside `useEffect`. |
| `session-one-door` | SKYFE016 | The session token is written only through `lib/session` (setter import or token-ish storage write elsewhere is flagged). |
| `guard-tristate` | SKYFE017 | A guard redirects on a tri-state `SessionState`, never a raw `isAuthenticated` boolean. |
| `route-param-guard` | SKYFE018 | A route reading a required id param through a loose `useParams()` guards its absence with a declarative redirect. |
| `safe-back` | SKYFE019 | No bare `history.back()`/`navigate(-1)` — use `safeBack`/`useGoBack` with a fallback. |
| `no-hardcoded-base-url` | SKYFE020 | The API base URL comes from configuration, not a literal host in the client's construction. |
| `no-raw-html` | SKYFE021 | No `dangerouslySetInnerHTML` outside the one sanitizing seam (`lib/html`). |
| `no-open-redirect` | SKYFE022 | Never navigate to a value read from the URL without mapping it through an allowlist. |
| `query-client-defaults` | SKYFE027 | A production `QueryClient` carries the mutation defaults (`MutationCache` with `onSuccess` invalidation and `onError` feedback). |
| `no-manual-refetch-ritual` | SKYFE028 | No `onSuccess` whose only job is to refetch/invalidate what the defaults already invalidate. |
| `refresh-one-door` | SKYFE029 | Token refresh is consumed only by the client/session rotation seam. |
| `no-cast-navigation` | SKYFE030 | No `as never`/`as any`/`as unknown` on a navigation target (keeps typed routes on). |
| `submit-handles-invalid` | SKYFE031 | `handleSubmit` carries its invalid path (`submitOrReveal` or a second argument). |
| `controller-field-state` | SKYFE032 | A `<Controller>` render reads and surfaces `fieldState`. |
| `tests-live-in-specs` | SKYFE036 | A test (`test`/`it`/`describe` from a runner) lives under `.specs/<id>-<slug>/e2e/`. |

SKYFE015–019 recognize TanStack Router and React Router idioms; they police a shape, not a router runtime. The
`SessionState`, `safeBack` and `submitOrReveal` helpers they steer toward live in `@skiesjs/react`.

## Layout

`index.cjs` assembles the plugin from `rules/<rule-id>.cjs` (one file per rule); shared AST helpers live in
`lib/shared.cjs`. Rule ids are stable public API.

## Usage

```js
// eslint.config.mjs
import skies from "@skiesjs/eslint-plugin";

export default [
  skies.configs.recommended, // every SKYFE rule at "warn" + the jsx-a11y floor at "error"
  { files: ["src/**/*.{ts,tsx}"], rules: { "skies/view-purity": "error", "skies/data-door": "error" } },
];
```

## Accessibility floor

`recommended` also carries [`eslint-plugin-jsx-a11y`](https://www.npmjs.com/package/eslint-plugin-jsx-a11y)'s
recommended set at error, on by default. It is a dependency of this package, so there is nothing else to install
or register (registering `jsx-a11y` again in the same config is a conflict). `jsx-a11y/aria-role` runs with
`ignoreNonDOM: true`. Relax one rule explicitly in a later config object:
`{ files: [...], rules: { "jsx-a11y/no-autofocus": "off" } }`.

## Tests

`npm test` runs `index.test.cjs`: RuleTester cases that pin each rule on both edges (fires on the violation, passes
on the allowed shapes), plus `a11y.test.cjs`, which pins the accessibility floor in `recommended`.
