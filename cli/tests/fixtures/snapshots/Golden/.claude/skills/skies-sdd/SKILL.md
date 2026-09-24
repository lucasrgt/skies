---
name: skies-sdd
description: Spec-driven delivery for Skies apps — write the failure modes, then red E2E, then the code, then record a receipt. Use for any "add a feature / endpoint / screen / flow" or bug fix with observable behavior in a Skies app.
---

# Skies spec-driven delivery

A feature is done when its spec folder holds a receipt showing every failure mode failing before the change and
passing after it. CI runs the cases on every push from then on. Nothing else is required: no tags, no manifests, no
gate.

```
.specs/<id>-<slug>/
  spec.md        behavior + failure modes (FM-1..n) + out of scope
  e2e/           black-box tests, each case titled "FM-n: …"
  receipt.json   written by `skies proof record`
  evidence/      artifacts tests save to $SKIES_EVIDENCE (committed); raw/ keeps full reports (local)
```

Three commands: `skies proof impact` (what a change reaches), `skies proof run` (the cases, now), and
`skies proof record` (red then green, once, into the receipt).

**Write first, commit together.** The spec and its E2E are written before the code, but they are committed with
the code that makes them pass: one change holds the spec, its E2E, the feature, and the receipt, so every commit
stays green. Never commit a failing spec on its own. Red is proven by `skies proof record` against the merge-base,
not by a red commit.

## 1. Understand

Read the module's `<Module>.ctx.md` and the slices or screens the change touches. Check `Skies.toml` for the
runners available (`[runners.*]`). If the request is ambiguous about behavior, ask before writing the spec.

Then find the specs your change can break:

```bash
skies proof impact <files or folders you expect to touch>
```

It names the `<Module>.ctx.md` of every module the paths reach and prints the specs its design notes cite, with
their failure modes, plus every spec whose `touches:` matches a path. Those modes are behavior other features rely
on; yours must contradict none of them. Their cases run in CI (and with `skies proof run <id>` locally).

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

A spec for a screen or shared file outside `Modules/` can claim it with `touches:` in the frontmatter, so
`skies proof impact` finds the spec from that path:

```yaml
---
id: "0012"
runner: web
touches: [frontend/web/src/deposit/**]
---
```

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
- Assert the error **code**, not only the status. A route that does not exist also answers 404, so a not-found
  failure mode asserted on the status alone passes before the endpoint exists and after it is misrouted. Read the
  body's `code` (`wallets.not_found`) the way the client does.
- Seed through the app's own services or endpoints; never mock the thing the failure mode is about.
- For a tagged failure mode, run the Assay verifier against the real app and save its verdict before asserting:
  `SpecEvidence.Save("avp-FM-5.json", verdict)` in .NET (`Skies.Framework.Testing`; pass
  `transport: app.CreateClient` to `Runner.Run` to use the test host), or write `verdictToJsonLine(verdict)` to
  `$SKIES_EVIDENCE/avp-FM-5.json` in TypeScript. The mode passes only when the verdict does.
- For `idempotency-key-honored`, `IdField` names a top-level response field whose value changes with every
  application (a create's `id`, a deposit's resulting `balance`), never one that echoes the input; seed enough
  state for two applications.

Run them now with `skies proof run <id>`: each failure mode's pass or fail, with what the failing cases reported,
and nothing committed. Every mode must fail, for the right reason (missing endpoint, wrong status, missing effect):
a mode that passes now does not discriminate.

## 4. Implement

Generate the shapes, then write the behavior following the conventions. Keep `skies doctor` clean (it builds the
backend with the SKY analyzers, and lints and typechecks every React package).

- Backend: `skies g module|slice|entity|vo|crud|hub`, from the app root or the API project (`--project <dir>` when
  `Skies.toml` declares several backends). `g slice` maps the slice under its module's route group and inherits the
  group's authorization decision.
- Frontend: `skies g feature <Name> --kind list|form` (a read screen, or a command screen with its form, pending,
  error, and success states), `skies g client` after the contract moves, `skies i18n` after copy changes.

Rerun `skies proof run <id>` until every mode passes, and run the cases of the specs `impact` named.

An isolated system with many cases (a value object, a calculation, a parser) gets its own spec, never a loose unit
test: failure modes first, isolated cases in its `e2e/` titled `FM-n: …`, a `red.patch` that breaks the invariant,
and a receipt like any other spec.

## 5. Record the receipt

```bash
skies proof record <id>
```

It runs the E2E against the merge-base (every FM must fail) and against the working tree (every FM must pass),
then writes `receipt.json`: per FM, red and green, the cases that proved it, and what red's first failing case
said. What the tests saved to `$SKIES_EVIDENCE` is committed under `evidence/` (256 KB each at most); full reports
stay in the gitignored `evidence/raw/`. If the E2E cannot even build on the merge-base (it uses code the feature
adds), every FM counts as failing (`did-not-build`) and the receipt's `red.output` says why. If a failure mode
already passes on the merge-base, either the test does not discriminate (fix the test) or the behavior already
existed (add a `## Non-discriminating` section to `spec.md` explaining why).

The merge-base is taken with the default branch. If red resolves to the wrong revision (the app branches from
`develop`, not `main`), set `[workspace] default_branch = "develop"` in `Skies.toml` once, or pass `--red <rev>`.
For a spec written after the code, pass `--red-patch <file>` with a patch that removes the behavior (for example,
a stubbed handler); it is kept as the spec's `red.patch`.

## 6. Revise the module context

For every module the change touched, reread its `<Module>.ctx.md`. If an invariant changed or a new one appeared,
update `## Design notes` and cite the spec that proves it, as the backticked folder name with an optional failure
mode: "Overdraw is refused as a business rule (`` `0002-withdraw#FM-2` ``)." Remove a note whose invariant is gone.
Keep it prose: the why, not a list of files or routes. `skies doctor` (`SKY0005`) flags a citation whose spec or
failure mode does not exist, and the citations are what `skies proof impact` follows.

## 7. Report

Report the spec path, the failure modes (with their `[avp: …]` tags), the receipt summary (red/green per FM), the
specs `impact` named and whether their cases still pass, the ctx.md files you revised, and `skies doctor` status.
Then commit the spec, its E2E, the code, and the receipt together.
