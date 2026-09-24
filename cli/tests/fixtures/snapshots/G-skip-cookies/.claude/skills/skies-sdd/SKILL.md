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
  evidence/      report + artifacts tests save to $SKIES_EVIDENCE (hashed by the receipt)
```

## 1. Understand

Read the module's `<Module>.ctx.md` and the slices or screens the change touches. Check `Skies.toml` for the
runners available (`[runners.*]`). If the request is ambiguous about behavior, ask before writing the spec.

Then find the specs your change can break:

```bash
skies proof impact <files or folders you expect to touch>
skies proof verify <the impacted ids>      # baseline: they must pass before you change anything
```

`impact` reads every receipt's footprint and prints each impacted spec with its failure modes. Read them: they are
behavior other features rely on, and your failure modes must not contradict them. If a baseline verify already
fails, report it before starting; it is not yours to hide.

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

When a failure mode matches an archetype in the AVP catalog (request idempotency, authorization, money integrity,
pagination…), decide it with that Assay verifier instead of a hand-rolled check, and tag the line with the criterion
id: `- FM-5 A retry with the same key credits twice [avp: idempotency-key-honored]`. The tag is optional; use it
only when an archetype fits.

**Stop and show the failure modes to the human.** They are the point of review; the rest follows from them.

## 3. Write the E2E and watch it fail

In `e2e/`, write black-box tests that drive the feature from outside: HTTP against the booted app
(`SkiesWebTest<Program>`), Playwright against the web UI, `integration_test` (or Maestro) on Flutter.

Every test lives in a spec; the doctors flag a test anywhere else (`SKY0029`, `SKYFE036`, `SKYFL036`). Flutter
cases import `package:<app>/...` and the spec's runner copies them into the package's `test/.skies_spec/`.

- One or more cases per failure mode; each case title starts with its id: `[Fact(DisplayName = "FM-2: …")]`,
  `test("FM-2: …")`, `testWidgets('FM-2: …')`.
- .NET spec tests live in namespace `Specs.S<id>` so the runner filter selects exactly this spec.
- Assert the observable outcome and, for rejections, that state did not change.
- Seed through the app's own services or endpoints; never mock the thing the failure mode is about.
- For a tagged failure mode, run the Assay verifier against the real app and save its verdict before asserting:
  `SpecEvidence.Save("avp-FM-5.json", verdict)` in .NET (`Skies.Framework.Testing`; pass
  `transport: app.CreateClient` to `Runner.Run` to use the test host), or write `verdictToJsonLine(verdict)` to
  `$SKIES_EVIDENCE/avp-FM-5.json` in TypeScript. The mode passes only when the verdict does.

Run them now. Every case must fail for the right reason (missing endpoint, wrong status, missing effect).

## 4. Implement

Generate the shapes (`skies g module|slice|entity|vo|crud|hub|feature|client`), then write the behavior following
the conventions. Keep `skies doctor` clean.

An isolated system with many cases (a value object, a calculation, a parser) gets its own spec, never a loose unit
test: failure modes first, isolated cases in its `e2e/` titled `FM-n: …`, a `red.patch` that breaks the invariant,
and a receipt like any other spec.

## 5. Record the receipt

```bash
skies proof record <id> --with-impacted
```

It runs the E2E against the merge-base (every FM must fail) and against the working tree (every FM must pass),
then writes `receipt.json` and copies the report and saved artifacts into `evidence/`. A tagged FM passes only when
its cases pass and its verdict reports every tagged criterion as pass. `--with-impacted` then reruns green for every
other spec whose files overlap yours and names the ones that passed in `verified_with`; if one fails, your change
broke it: fix the code, not the other spec. Never edit `evidence/` by hand; `skies proof status` reports it as
tampered. If the E2E cannot even build on the merge-base
(it uses code the feature adds), every FM counts as failing and the build output is kept as `evidence/red.log`.
If a failure mode already passes on the merge-base, either the test does not discriminate (fix the test) or the behavior already existed (add a
`## Non-discriminating` section to `spec.md` explaining why).

For a spec written after the code, add a `red.patch` that removes the behavior (for example, stub the handler)
and record against it.

## 6. Report

Report the spec path, the failure modes (with their `[avp: …]` tags), the receipt summary (red/green per FM), the
impacted specs and whether they still pass (`verified_with`), and `skies doctor` status. If other receipts went
stale because of your change (`skies proof status`), say so and rerun them with `skies proof verify <ids>`.
