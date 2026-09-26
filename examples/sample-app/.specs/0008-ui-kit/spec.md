---
id: "0008"
runner: web
touches: [frontend/web/src/ui/theme.ts, frontend/web/src/ui/index.ts]
---
# The web UI kit

The sample's web kit (`frontend/web/src/ui`) is the one place a screen's interactive states, form anatomy, and
typography are decided, so no screen re-makes them. It is an isolated system: its cases render each primitive on
its own, with values read from the theme.

## Failure modes

- FM-1 A button's visible label is not its accessible name, or pressing it does not fire its action.
- FM-2 A loading or disabled button still fires its action, or a loading one is not announced (`aria-busy`).
- FM-3 A focused button shows no focus ring in place of the outline it suppresses, or keeps the ring after blur.
- FM-4 A field's label, hint, and error are not wired to its control (label association, `aria-describedby`, the
  error as `role=alert`, `aria-invalid`).
- FM-5 An input does not hand `onChange` the DOM change event with the typed value, or does not report its blur
  (`onBlur`), so a react-hook-form `field` cannot bind to it as it binds to a plain `<input>`.
- FM-6 Text does not map its role to the type scale and the document outline, its tone to a semantic color, or its
  alert flag to `role=alert`.
- FM-7 A stack does not space its children from the gap scale, or its children carry their own margin.
- FM-8 The empty state loses its copy, or the error state loses its retry action.
- FM-9 A primitive accepts `className` or `style`, reopening the prop surface to one-off styling.

## Non-discriminating

- FM-9 is a compile-time guarantee: its case holds `@ts-expect-error` lines that `npm run typecheck` fails on when
  a primitive grows `className` or `style`. At run time the case passes whatever the props are, so red cannot make
  it fail; the typecheck is what bites.

## Out of scope

The kit's look: the cases read every value from the theme, so a restyle is not a failure. Mobile is a Flutter app
with its own kit (`docs/FLUTTER-CONVENTIONS.md`).
