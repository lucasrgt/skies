<!-- skies-node:foundations:start -->
## Skies Node foundation workflow

The primary coding agent owns the complete foundation lifecycle. Never create or delegate one agent per foundation.

1. At task start, run `skies-node-foundation context --task "<goal>" --path <expected-path>`. Treat every returned decision, invariant, way, scar, and due deferment as governing context.
2. Rerun `skies-node-foundation context` after scope changes, context compaction, or movement into an unfamiliar area. Keep retrieval bounded with accurate task text and paths.
3. Use the repository-local foundation skills only when a real lifecycle event occurs: accepted decisions for WTW, proven patterns for RTW, corrected failures for NYA, or evidence-backed conditional deferments for NWC. Never record hypothetical guidance.
4. Run focused repository tests and linters during implementation.
5. Before commit, stage the exact intended paths and run `skies-node-foundation check --task "<completed work>" --staged`. Staged checks are always bounded: mapped proofs run, while exhaustive fallbacks and browser/device execution wait for authoritative CI.
6. Before push, run `skies-node-foundation check --task "<review>" --base <target-revision> --fast`. The pre-push hook repeats this bounded committed-diff review.
7. Never replace the automation-owned depth gates: pull-request CI runs affected without --fast, and release automation runs --full. Do not report an external delivery complete until its required status is green. Bare `skies-node-foundation check --task ...` is intentionally invalid so an ambiguous scope cannot start a surprise exhaustive run.
8. Allow one automatic authoritative/full attempt. After failure or interruption, stop and check whether you are in a loop. Diagnose the first failure, correct its cause and run focused verification before one retry with `--retry-review <json-file>` containing PreviousAttemptId, Diagnosis, Correction and FocusedVerification. Never launch overlapping broad checks. Exit 1 means findings and exit 2 or greater means incomplete validation. Neither is a pass; focused checks do not replace the required gate.

Tests, linters, review, and individual foundation commands do not replace `skies-node-foundation check`.
<!-- skies-node:foundations:end -->
