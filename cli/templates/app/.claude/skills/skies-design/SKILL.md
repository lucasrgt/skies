---
name: skies-design
description: Use when creating or modifying any UI in an Skies app — screens (*.view.tsx), ui/ primitives, forms, or visual styling. Loads the design constitution, recalls the Assay Design contract, then enforces the exit ritual (lint + render + visual self-review).
---

# skies-design — build UI by instantiation, never invention

This skill carries no rules of its own. It routes you to the truth (the constitution, the
recipes, the design contract) and holds you to the exit ritual. If anything here disagrees with
`docs/DESIGN-CONVENTIONS.md`, the constitution wins.

## 1. Load (before writing any JSX)

- Run `npx assay-design recall --contract .design/contract.toml --task "<what you are building>"`
  (add `--path <file>` when touching a known screen). Obey the returned `rules`, scales, and
  component vocabulary — do not invent off-scale values or undeclared parts.
- Read `docs/DESIGN-CONVENTIONS.md` — the taxonomy section plus the section matching the task
  (form anatomy / text hierarchy / layout & spacing / color discipline).
  In a pilot app, the constitution lives in the framework checkout declared by
  `Skies.toml [framework] repo`.
- Open the recipe the constitution's recipe index maps to this screen archetype, and read its
  four files (view, viewModel, test, i18n) — they are real, compiled, lint-clean code.

## 2. Build

- Instantiate the recipe: copy its structure, rename, bind your slice. Do not compose a screen
  from a blank file.
- The vocabulary is closed: kit primitives + token props only. A missing primitive is extended
  in YOUR `ui/` following the constitution — never inlined paint (SKYFE024–026 will catch it;
  don't make them).
- Token values are the app's; token names and the kit API are not yours to fork.
- Keep the `data-ui` evidence seam on kit primitives (`data-ui`, `data-ui-slot`, variants/states).

## 3. Verify — the exit ritual, all three, in order

1. The app's check command is green (`npm run check` runs `design:doctor`; after a surface exists,
   also `design:audit` / Evidence IR `check` when wired).
2. Render the screen; take a screenshot and look at it.
3. Self-review the screenshot against five points — fix and re-render until it reads clean:
   - visual hierarchy: one title, roles descending without skipping;
   - spacing rhythm: token scale only, no margin hacks;
   - the five states on every interactive (rest/hover/focus-visible/active/disabled);
   - contrast pairs: `onPrimary`/`onDanger` on their fills, AA on text;
   - form anatomy: label → control → hint|error, command error as the alert block.
