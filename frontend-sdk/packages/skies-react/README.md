# @skiesjs/react

The frontend **spine** of [Skies](https://github.com/lucasrgt/skies) —
router- and design-system-agnostic React primitives the app composes into MVVM screens. It ships the shapes
the framework's frontend conventions (`SKYFE*`) steer toward, so the rules have something to steer *to*:

- **`AsyncState` / `<Resource>`** — the one async-state shape (loading / error / data), never raw `isPending`/`isError`.
- **Session** — `SessionState` / `toSessionState` (the tri-state `loading | authenticated | anonymous`) + the
  `lib/session` seam that pairs the bearer write with a cache reset.
- **Navigation** — `safeBack` and guarded redirect helpers (router-agnostic: TanStack Router ↔ React Router).
- **Paging** — `usePager` / `useAccumulatedPages` over a stable, tiebroken order.
- **Submit** — a typed submit helper for form → command flows.

Stranger-maintainable, doctor-removable: plain React you can read, with no runtime you inherit from. The only
peer dependency is `react` (>=18).

```bash
npm install @skiesjs/react
```

```ts
import { Resource, toSessionState, safeBack } from "@skiesjs/react";
```

MIT © Lucas Tinoco
