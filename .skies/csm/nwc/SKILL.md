---
name: now-we-can
description: Collect, wake, resolve, and enforce evidence-backed conditional deferments with CSM-managed NWC. Use automatically at task and context boundaries, after producing plans or roadmaps, after a completed agent response, and before completion.
---

# Now We Can

1. The primary agent runs `dotnet tool run skies context --task "<goal>"` at
   task start and after context reset or compaction. This wakes due deferments
   together with the other foundations. Do not assign a separate permanent
   agent to NWC.
2. Use `dotnet tool run skies nwc resolve --id <id> --evidence "<proof>"` only after completing the
   deferred action.
3. The host runs `dotnet tool run skies nwc collect --task "<goal>" --plan <file> --final-message <file>`
   after work. Do not create deferments manually.
4. A deferment requires a concrete future action, a currently false blocker, a
   machine-checkable cue, reusable scope, and verbatim evidence from the task,
   plan, final response, or diff.
5. Reject aspirations, optional polish, unfinished current scope, permanent
   fallbacks, vague "later" language, already completed work, and invented
   paths or events.
6. Before commit, the primary agent stages the intended paths and runs
   `dotnet tool run skies check --task "<completed task>" --staged`. Pull-request
   CI owns authoritative affected verification and release automation owns
   `--full`. Use `dotnet tool run skies nwc wake` or
   `dotnet tool run skies nwc check` only for focused event delivery,
   investigation, or maintenance. Exit code 1 requires resolving every due
   deferment; exit code 2 is not a pass.

Collection and delivery are harness responsibilities. Never rely on voluntary
agent recall.
