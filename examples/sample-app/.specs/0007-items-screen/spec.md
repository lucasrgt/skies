---
id: "0007"
runner: web
touches: [frontend/web/src/items/Items.view.tsx, frontend/web/src/items/items.i18n.ts]
---
# The Items screen

The Items screen lists items through one `AsyncState` resource, so the View renders loading, error, empty, and
success by construction. An empty list is its own state with its own copy, not a success with nothing in it.

## Failure modes

- FM-1 The list resource starts in any state but loading while the list is fetched (the View flashes an empty or
  stale screen).
- FM-2 A list that settles empty does not render the kit's empty state.

## Out of scope

The error branch and its retry: the spine's `<Resource>` renders them, and its own package tests pin that.
