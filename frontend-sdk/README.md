# Skies Framework — frontend

The React web side of Skies (mobile is Flutter: `flutter-sdk/`): a small runtime (`@skiesjs/react`) and architecture-only lint rules
(`@skiesjs/eslint-plugin`). Plain React the app composes, not a DSL; delete either package and the app still builds.
Scaffolding, client generation and i18n assembly live in the `skies` CLI, not here.

## Layout

```
frontend-sdk/
  package.json        # private npm workspace root
  tsconfig.base.json  # strict TS config every package/sample extends
  eslint.config.mjs   # lints the canonical sample with the SKYFE rules
  vitest.config.ts    # jsdom + the @/… aliases that run the sample against source
  packages/
    skies-react/      # @skiesjs/react — AsyncState/Resource, session seam, guards, nav, params, submit, paging
    eslint-plugin/    # @skiesjs/eslint-plugin — SKYFE rules, one file per rule, RuleTester self-tests
../examples/sample-app/frontend/web/
  src/<feature>/      # ViewModel + View + i18n catalog per feature
  src/client.gen/     # the generated client
  src/ui/             # the app's own component kit
```

`npm run check` = typecheck + lint (sample + rule self-tests) + vitest.

## The runtime — `@skiesjs/react`

A screen's **ViewModel** (the data door) exposes its resource as an `AsyncState<T>`; the **View** renders it through
`<Resource>`, so loading / error / empty are handled by construction.

```ts
type AsyncState<T> =
  | { status: "loading" }
  | { status: "error"; message: string; retry?: () => void }
  | { status: "empty" }
  | { status: "ready"; data: T };
```

`toAsyncState(query, { errorMessage, isEmpty? })` projects a react-query result into the union; react-query still
owns fetching and caching. The package also ships the tri-state `SessionState`, the session seam and
`singleFlight` refresh, `guardSession`, `safeBack`, `requiredParam`, `submitOrReveal` and the paging hooks.

## The feature unit

| File | Role |
|---|---|
| `<Feature>.viewModel.ts` | the only importer of the generated client; renders nothing; exposes `AsyncState` + commands |
| `<Feature>.view.tsx` | render only; consumes the ViewModel through `<Resource>` |
| `<feature>.i18n.ts` | per-feature copy, every locale with the same keys |
| `<Feature>.test.tsx` | optional colocated tests |

## The rules — `@skiesjs/eslint-plugin`

Architecture only: View purity, one data door, no mocks in production, platform-agnostic ViewModels, state through
`<Resource>`, complete i18n and no hardcoded copy, plus the routing/session/form seams. See
[`packages/eslint-plugin/README.md`](packages/eslint-plugin/README.md) for the rule list.
