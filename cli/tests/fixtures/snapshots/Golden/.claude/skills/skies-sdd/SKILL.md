---
name: skies-sdd
description: Spec-driven delivery for Skies apps — write the failure modes, then red E2E, then the code, then record a receipt. Use for any "add a feature / endpoint / screen / flow" or bug fix with observable behavior in a Skies app.
---

# Skies spec-driven delivery

A feature is done when its spec folder holds a receipt showing every failure mode failing before the change and
passing after it. Nothing else is required: no tags, no manifests, no gate.

```
.specs/<id>-<slug>/
  spec.md        behavior + failure modes (FM-1..n) + out of scope
  e2e/           black-box tests, each case titled "FM-n: …"
  receipt.json   written by `skies proof record`
  evidence/      report + small artifacts the runner saves to {evidence}
```

## 1. Understand

Read the module's `<Module>.ctx.md` and the slices or screens the change touches. Check `Skies.toml` for the
runners available (`[runners.*]`). If the request is ambiguous about behavior, ask before writing the spec.

## 2. Write the spec (before any code)

Run `skies spec new <slug> [--runner <name>]` and fill `spec.md`:

- **What it does**, in two to five lines, in product terms.
- **Failure modes**: one line per way the feature can be wrong, observable from outside. Each line starts with
  `- FM-n`. Write what would go wrong, not how you will test it.
- **Out of scope**: what this spec deliberately does not cover.

Walk this checklist and keep only what applies:

| Area | Ask |
|---|---|
| Real effect | Does the action produce its effect, and does a failure report failure (never a false success)? |
| Real data | Could the screen render fixtures, stale data, or invented rows? |
| Authorization | Can a caller act on a resource it does not own, or with the wrong role? Is an anonymous call refused? |
| Preconditions | Is each state transition refused server-side when its preconditions do not hold? |
| Validation | Is invalid input refused, every invalid field reported, and state left untouched? |
| Idempotency | Does a retry with the same key apply at most once? |
| Concurrency | Do concurrent writes surface a conflict instead of silently losing one? |
| Uniqueness | Is a duplicate refused server-side? |
| Side effects | Does a transition fire every downstream effect (notifications, emails, ledger rows)? |
| Money | Are amounts exact at rest and on screen; do splits sum to the whole? |
| Time | Are times shown in the user's zone and date-only values never shifted? |
| Paging | Does paging the whole set return every item exactly once? |
| Navigation | Does every affordance lead somewhere real, and does a role see only its routes? |
| Resilience | Does bad or missing data degrade gracefully instead of crashing the screen? |
| Sessions | Do tokens rotate and expire as promised; are wrong credentials denied? |
| Callbacks | Are webhooks verified and bound to the right environment? |

**Stop and show the failure modes to the human.** They are the point of review; the rest follows from them.

## 3. Write the E2E and watch it fail

In `e2e/`, write black-box tests that drive the feature from outside: HTTP against the booted app
(`SkiesWebTest<Program>`), Playwright or Maestro against the UI, `integration_test` on Flutter.

- One or more cases per failure mode; each case title starts with its id: `[Fact(DisplayName = "FM-2: …")]`,
  `test("FM-2: …")`, `testWidgets('FM-2: …')`.
- .NET spec tests live in namespace `Specs.S<id>` so the runner filter selects exactly this spec.
- Assert the observable outcome and, for rejections, that state did not change.
- Seed through the app's own services or endpoints; never mock the thing the failure mode is about.

Run them now. Every case must fail for the right reason (missing endpoint, wrong status, missing effect).

## 4. Implement

Generate the shapes (`skies g module|slice|entity|vo|crud|hub|feature|client`), then write the behavior following
the conventions. Keep `skies doctor` clean. Add a unit test only for an isolated system (a value object, a
calculation), and only after writing its failure modes.

## 5. Record the receipt

```bash
skies proof record <id>
```

It runs the E2E against the merge-base (every FM must fail) and against the working tree (every FM must pass),
then writes `receipt.json` and copies the report into `evidence/`. If a failure mode already passes on the
merge-base, either the test does not discriminate (fix the test) or the behavior already existed (add a
`## Non-discriminating` section to `spec.md` explaining why).

For a spec written after the code, add a `red.patch` that removes the behavior (for example, stub the handler)
and record against it.

## 6. Report

Report the spec path, the failure modes, the receipt summary (red/green per FM), and `skies doctor` status. If
other receipts went stale because of your change (`skies proof status`), say so and rerun them with
`skies proof verify <ids>` when the change could affect them.
