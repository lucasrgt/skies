# @skiesjs/foundation

Fail-closed proof inventory, criteria matrix, affected gates, and repository-local CSM foundations for plain Node.js applications. The package is strict NodeNext ESM and has no runtime dependencies. Its executable is `skies-node-foundation`.

## Closed proof manifest

Put `skies.node.json` at the workspace root. Unknown keys, references, proof kinds, duplicate IDs/citations, unsafe paths, invalid timeouts, and dependency cycles are configuration errors.

```json
{
  "schemaVersion": 1,
  "git": { "base": "origin/main" },
  "criteria": [
    { "id": "wallet.withdraw.no-overdraw", "statement": "A withdrawal cannot overdraw the wallet." },
    { "id": "wallet.withdraw.audited", "statement": "A successful withdrawal is observable in the audit trail." }
  ],
  "lanes": [
    {
      "id": "wallet-unit",
      "command": ["npm", "exec", "vitest", "run", "src/wallet/withdraw.test.ts"],
      "timeoutMs": 60000,
      "cwd": ".",
      "env": { "NODE_ENV": "test" }
    },
    {
      "id": "wallet-journey",
      "command": ["npm", "exec", "playwright", "test", "e2e/withdraw.spec.ts"],
      "timeoutMs": 180000
    }
  ],
  "proofs": [
    {
      "id": "withdraw-unit",
      "kind": "unit",
      "lane": "wallet-unit",
      "criteria": ["wallet.withdraw.no-overdraw"],
      "sourceScopes": ["src/wallet/**"],
      "description": "Domain proof"
    },
    {
      "id": "withdraw-journey",
      "kind": "journey",
      "lane": "wallet-journey",
      "criteria": ["wallet.withdraw.audited"],
      "sourceScopes": ["src/wallet/**", "e2e/**"],
      "dependsOn": ["withdraw-unit"]
    }
  ],
  "ignoreScopes": ["docs/**"],
  "forceFullScopes": ["package.json", "package-lock.json", "skies.node.json"]
}
```

Proof kinds are closed to `unit`, `integration`, `e2e`, and `journey`. Commands are argv arrays: they never pass through a shell. Each selected lane runs once. A missing executable, timeout, signal, nonzero exit, missing proof, uncovered criterion, or unknown impact is red. Unaffected obligations remain `not-affected`, never `pass`.

An ignored scope applies only if no proof or force-full scope matches the path. Any other unmapped changed path widens to every proof. `dependsOn` adds a deterministic transitive proof closure.

## Inventory, criteria, and matrix

```sh
skies-node-foundation inventory [--json]
skies-node-foundation criteria check [--json]
skies-node-foundation matrix [--json]
skies-node-foundation matrix --receipt .skies/foundation/gate-receipt.json [--json]
```

`inventory` shows commands, timeouts, citations, kinds, and source scopes. `criteria` requires every declared criterion to have at least one cited proof. A matrix without a receipt reports static `covered`/`no-proof` states, not execution passes. With a receipt it recomputes `pass`, `fail`, `not-run`, `not-affected`, and `no-proof` from the current manifest; a stale, foreign, duplicate, or unknown receipt fails closed.

## Gate modes and receipts

```sh
# Explicit paths, useful to hooks and orchestration
skies-node-foundation gate --affected --changed src/wallet/withdraw.ts --changed src/shared/clock.ts

# Git merge-base of git.base and HEAD, plus staged, unstaged, and untracked paths
skies-node-foundation gate --affected
skies-node-foundation gate --affected --merge-base origin/release

# Pre-push scope: committed diff base...HEAD, exhaustive fallbacks deferred to CI
skies-node-foundation gate --base origin/main --fast

# Staged index diff, always bounded
skies-node-foundation gate --staged

# Every proof and canonical release artifacts (release automation only)
skies-node-foundation gate --full
```

Affected, base, and staged gates write `.skies/foundation/gate-receipt.json`. Full writes `VERIFICATION.json` and `VERIFICATION.md`. Override with `--report <path>` and `--markdown <path>`, or use `--no-report`. `--fast` defers force-full and unmapped-path widening to authoritative CI; `--full` and `--fast` conflict. JSON receipts contain the configuration SHA-256, mode/base, normalized changed paths, selection reasons, argv/cwd/timeout, bounded stdout/stderr, exit/signal/timeout/duration, every proof outcome, criteria matrix, findings, and overall verdict. Missing Git ancestry fails the scoped check without executing lanes, including in fast mode. Fast force-full deferral preserves directly mapped proofs and their dependencies. Native processes keep argv boundaries on Windows, and timeouts terminate their process trees.

The process runner forwards `SIGINT`, `SIGTERM`, and `SIGHUP` to the active process group, terminates timed-out processes, never enables a shell, and caps captured output. Use the exported `CommandRunner`, `GitClient`, and clock seams for deterministic embedding/tests.

## Repository-local CSM foundations

```sh
skies-node-foundation foundations init [--dry-run] [--agent-file AGENTS.md]
skies-node-foundation foundations sync [--dry-run]
# equivalent structured form
skies-node-foundation foundation stack init
```

The stack creates `csm.toml` (the shared CSM storage contract, mirroring the .NET side), a pinned lock, managed Node-focused `SKILL.md` surfaces, versioned empty stores, and one managed protocol block in detected agent files. A legacy `csm.json` is still read for migration but never written. A custom nested agent file also installs the portable root `AGENTS.md` surface. Init refuses conflicting managed files; sync updates only recognized owned assets. Plans are preflighted and applied transactionally. Parent traversal, absolute paths, non-files, and symlinks are rejected. Re-running either operation is idempotent.

CSM records are ordinary formatted JSON text beneath the configured storage root:

```sh
skies-node-foundation wtw collect --kind invariant --id server-authority \
  --title "Server authority" --statement "The server owns totals." \
  --violation "A client total is accepted."
skies-node-foundation wtw explain
skies-node-foundation wtw guard

skies-node-foundation rtw add --id co-located-tests --title "Co-locate tests" \
  --guidance "Tests stay below src."
skies-node-foundation rtw guide
skies-node-foundation rtw check

skies-node-foundation nya spec --id magic-color --title "Magic color" \
  --lesson "Use a semantic design token."
skies-node-foundation nya recall
skies-node-foundation nya replay
skies-node-foundation nya check

skies-node-foundation nwc collect --id retry-work --title "Retry work" \
  --action "Finish retry handling."
skies-node-foundation nwc wake
skies-node-foundation nwc resolve --id retry-work
skies-node-foundation nwc check
```

Mutations accept `--dry-run`; all operations accept `--root`, and result/list/check operations accept `--json`.

## Bounded workflows

```sh
skies-node-foundation context --task "Add retry status" \
  --path src/status.ts --event dependency:retry-ready --limit 8

skies-node-foundation check --task "Review retry status" --affected
skies-node-foundation check --task "Pre-commit review" --staged
skies-node-foundation check --task "Pre-push review" --base origin/main --fast
skies-node-foundation check --task "Release audit" --full
# equivalent:
skies-node-foundation foundation workflow context --task "Add retry status"
skies-node-foundation foundation workflow check --task "Release audit" --full
```

`context` reads a bounded WTW → RTW → NYA → NWC view and performs no network or installation mutation. `check` always runs gate → WTW → RTW → NYA → NWC, preserving later findings after an earlier red step. Unlike the standalone gate default, workflow check requires an explicit scope.

## Exit codes

- `0`: successful command (an empty affected change is labeled `no-changes`, not green)
- `1`: gate, proof, criteria, record, or foundation finding
- `2`: invalid invocation, manifest/configuration, unsafe filesystem state, or incomplete inspection

Use `skies-node-foundation --help` for the complete command summary.

Authoritative and full CLI checks allow one automatic attempt. After failure or interruption, stop and check
whether you are in a loop. Diagnose the first failure and verify a focused correction before retrying with
`--retry-review <json-file>`. Include `PreviousAttemptId`, `Diagnosis`, `Correction`, and `FocusedVerification`
strings. The ID is in `.skies/verification-attempt/attempt.json`. Each review permits only one retry and does
not waive any proof or independently certify the supplied evidence. Fast and staged checks remain available.
An exclusive lock blocks simultaneous broad commands. After a killed process, verify that the PID in
`active.lock` has stopped before removing that lock; keep the attempt receipt and provide a retry review.
The guard covers CLI checks, not arbitrary external commands or model token usage.
