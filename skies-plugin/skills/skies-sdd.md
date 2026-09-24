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
  evidence/      artifacts tests save to $SKIES_EVIDENCE (committed, hashed); raw/ keeps full reports (local)
```

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
skies proof verify <the impacted ids>      # baseline: they must pass before you change anything
```

`impact` reads every receipt's footprint and prints each impacted spec with its failure modes, then the
`<Module>.ctx.md` of every module the paths reach. Read both before writing failure modes: the specs are behavior
other features rely on, the ctx holds the module's invariants and the specs that prove them, and your failure modes
must contradict neither. If a baseline verify already fails, report it before starting; it is not yours to hide.
`verify` only reruns green: a receipt that is current and still passes is left untouched (`--refresh` rewrites its
green evidence anyway), a stale one gets fresh green evidence. A spec with no receipt yet reads `unrecorded` in
`skies proof status` and in `verified_with`; record it before relying on it.

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

**`touches:` in the frontmatter** pins files the receipt must depend on that its runner cannot see. A receipt's
footprint is what the green run executed (with coverage) or the files changed since red (without it), plus
`touches`. List paths or globs, relative to the app root, when a file the spec relies on would otherwise stay out of
it: a View, a copy catalog, or a shared component a web runner without coverage renders but the change did not edit;
a config, JSON, or template read at runtime that coverage does not count. Each match is hashed whole, and a new file
matching a glob also makes the receipt stale:

```yaml
---
id: "0012"
runner: web
touches: [frontend/web/src/deposit/Deposit.view.tsx, frontend/web/src/deposit/deposit.i18n.ts]
---
```

A runner's `scope` in `Skies.toml` (`scope = ["frontend/web/"]`) bounds all of this: only files under it count for
that runner's specs (the diff, `touches` matches, coverage, and ctx notes), so a backend edit never stales a web spec.

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
- For `idempotency-key-honored`, `RequestIdempotencySubject`'s `IdField` names the top-level response field that
  identifies the effect: the verifier sends the body three times (key A, key A again, key B) and requires the same
  value on the replay and a different one under the new key. A create returns a fresh `id`; an in-place mutation
  names an outcome that moves with every application, like the resulting `balance`. With a multi-field output pick
  the field that changes each time (a transfer's `fromBalance`), never one that echoes the input (`fromWalletId`
  is the same under every key, so the verifier reports the keys as collapsed), and seed enough state for two
  applications.

Run them now, with `skies proof run <id>`: it runs the spec's E2E once on the working tree, prints each failure
mode's pass or fail with what the failing cases reported, and commits nothing (its report stays in the gitignored
`evidence/raw/`). Every mode must fail, for the right reason (missing endpoint, wrong status, missing effect): a
mode that passes now does not discriminate.

## 4. Implement

Generate the shapes, then write the behavior following the conventions. Keep `skies doctor` clean (it builds the
backend with the SKY analyzers, and lints and typechecks every React package).

- Backend: `skies g module|slice|entity|vo|crud|hub`, from the app root or the API project (`--project <dir>` when
  `Skies.toml` declares several backends). `g slice` maps the slice under its module's route group and inherits the
  group's authorization decision.
- Frontend: `skies g feature <Name> --kind list|form` (a read screen, or a command screen with its form, pending,
  error, and success states), `skies g client` after the contract moves, `skies i18n` after copy changes.

Rerun `skies proof run <id>` until every mode passes.

An isolated system with many cases (a value object, a calculation, a parser) gets its own spec, never a loose unit
test: failure modes first, isolated cases in its `e2e/` titled `FM-n: …`, a `red.patch` that breaks the invariant,
and a receipt like any other spec.

## 5. Record the receipt

```bash
skies proof record <id> --with-impacted
```

It runs the E2E against the merge-base (every FM must fail) and against the working tree (every FM must pass),
then writes `receipt.json` (per FM, the cases that decided it and, on red, what failed), commits the artifacts the
tests saved to `evidence/` (256 KB each at most), and keeps the full reports in the gitignored `evidence/raw/`. A
tagged FM passes only when its cases pass and its verdict reports every tagged criterion as pass. `--with-impacted`
then reruns green for every other spec whose files overlap yours and names the ones that passed in `verified_with`;
if one fails, your change broke it: fix the code, not the other spec. Never edit `evidence/` by hand; `skies proof
status` reports it as tampered. If the E2E cannot even build on the merge-base (it uses code the feature adds), every
FM counts as failing, for every runner, and the receipt's `red.output` says why (the whole output is
`evidence/raw/red.log`). If a failure mode already passes on the merge-base, either the test does not discriminate
(fix the test) or the behavior already existed (add a `## Non-discriminating` section to `spec.md` explaining why).

The merge-base is taken with the default branch. If red resolves to the wrong revision (the app branches from
`develop`, not `main`), set it once in `Skies.toml`, `[workspace] default_branch = "develop"`, or pass `--red <rev>`.

For a spec written after the code, add a `red.patch` that removes the behavior (for example, stub the handler)
and record against it. When `skies proof status` says `red-rotted`, the code under the patch moved and red can no
longer be reproduced: stub the behavior again, save `git diff --relative` as the spec's `red.patch`, restore the
code, and run `skies proof record <id> --red-only`, which reruns red alone and keeps green.

## 6. Revise the module context

For every module the change touched, reread its `<Module>.ctx.md`. If an invariant changed or a new one appeared,
update `## Design notes` and cite the spec that proves it, as the backticked folder name with an optional failure
mode: "Overdraw is refused as a business rule (`` `0002-withdraw#FM-2` ``)." Remove a note whose invariant is gone.
Keep it prose: the why, not a list of files or routes. `skies doctor` (`SKY0005`) flags a citation whose spec or
failure mode does not exist.

The note `skies proof record` printed in step 5 ("Wallets.ctx.md was not revised in this change; …") names each
touched module whose ctx stayed as it was; it never fails. A ctx is not part of any footprint, so revising it after
recording stales nothing; record again if you want the receipt's `ctx_revised` to list the revision.

## 7. Report

Report the spec path, the failure modes (with their `[avp: …]` tags), the receipt summary (red/green per FM), the
impacted specs and whether they still pass (`verified_with`), the ctx.md files you revised, and `skies doctor`
status. If other receipts went stale because of your change (`skies proof status`), say so and rerun them with
`skies proof verify <ids>`. Then commit the spec, its E2E, the code, and the receipt together.
