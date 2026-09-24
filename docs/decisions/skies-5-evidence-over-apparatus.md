# Decision: Skies 5 — evidence over apparatus

**Status:** accepted.
**Date:** 2026-09-23.
**Supersedes:** fail-closed verification, explicit verification boundaries, and primary-agent foundation
orchestration (removed from this folder; see git history). Full plan: [`.specs/0008-skies-5/techspec.md`](../../.specs/0008-skies-5/techspec.md).

## Context

Skies 4 answered "is this feature done?" with a verification gate. The gate selected tests from a Git diff through
static impact analysis in three languages, demanded a criterion per slice in `<Module>.spec.toml`, an `[AVP]` proof
per criterion, happy and sad `[Journey]` tests per write, `@verify`/`@avp`/`@e2e` tags per ViewModel, a
`flows.json` entry per flow, and a retry-review JSON after a failed run. Measured on the framework and on Hostpoint:

- about 4.2k of the CLI's 8.6k lines were gate and manifest code, against about 2.3k lines of .NET runtime;
- over 60 days, gate/CLI/analyzer churn was about 7x the runtime churn;
- Hostpoint's pre-push hook started Docker and an emulator for every push, and its last recorded attempt was
  left failed with an orphaned lock;
- the proofs it collected were often ceremonial: happy journeys with no assertion, criteria ids invented inside
  the test that proves them, E2E "evidence" that was UI copy, spec criteria that appeared in no test at all.

The checker became the target. Agents learned to satisfy it, and every new loophole produced another rule.

## Decision

1. **The doctor enforces architecture only.** Rules that demand the existence of tests, tags, criteria, or
   manifests are removed (SKY0003, 0008, 0010, 0011, 0020, 0030–0033; SKYFE005, 006, 008, 033–035; the Flutter
   equivalents). The design band and token taxonomy are removed: styling belongs to each product.
2. **Features are accepted by a receipt in their spec folder.** `.specs/<id>-<slug>/` holds `spec.md` with failure
   modes written before the code, black-box E2E in `e2e/`, and `receipt.json` from `skies proof record`, which
   requires every failure mode to fail on the red revision and pass on the working tree. The test title (`FM-n`) is
   the only link between spec and test.
3. **No gates.** The framework installs no hook and runs nothing automatically. A receipt stores the hashes of the
   files the feature touched; `skies proof status` shows which receipts went stale and `skies proof verify` reruns
   them when someone decides it matters.
4. **One Rust binary for all tooling.** Scaffolders, the doctor orchestrator, the Flutter rules, and the proof
   engine are a single `skies` executable. C# remains only where it runs inside .NET (libraries, Roslyn analyzers);
   TypeScript and Dart only for runtime spines and the ESLint plugin.
5. **Out of the framework:** the Node.js SDK, cross-runtime parity manifests, CSM foundations (why-this-way,
   right-this-way, not-you-again, now-we-can remain separate tools), framework-sync, and package version pinning.
6. **One version for every package**, released together.

## Consequences

- A regression surfaces when someone reruns a stale receipt, not on every push. This is deliberate: speed and
  clarity over an automatic net that had stopped catching real bugs.
- The receipt footprint is the set of files changed by the feature. A change to a shared file used by, but not
  changed in, the feature does not mark its receipt stale. Coverage-based footprints are future work.
- Review moves to where it pays: a human reads the failure modes in `spec.md` before code exists.
