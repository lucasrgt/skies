---
name: right-this-way
description: Find, follow, record, and verify repository-specific implementation patterns with CSM-managed RTW. Use at task start, after scope or context changes, when creating code analogous to existing features, after establishing a reusable reference implementation, and before commit, pull request, push, review, or completion.
---

# Right This Way

1. The primary agent runs `dotnet tool run skies context --task "<goal>"` before implementation and after scope or context changes. This retrieves relevant ways together with every other foundation. Do not assign a separate permanent agent to RTW.
2. Use `dotnet tool run skies rtw guide --task "<goal>" --path <expected-path>` only for deeper pattern inspection. Read every returned way and inspect each referenced file.
3. Preserve the invariants and structure described by relevant ways. Adapt names and domain details to the current task. Never copy a reference mechanically.
4. Run `dotnet tool run skies rtw add` only after a reusable pattern exists in tracked repository code. Provide a precise intent, actionable guidance, reusable scopes and tags, and at least one reference.
5. Before committing an uncommitted final diff, the primary agent stages the intended paths and runs `dotnet tool run skies check --task "<completed task>" --staged`.
6. For committed review, use the repository's checked authority: CI-authority pre-push uses `--base <target-revision> --fast`, while local-authority pre-push omits `--fast` and owns the affected verdict. Both expose an explicit `--full` release command.
7. Rerun the applicable check after every change to the reviewed diff. Exit code 1 requires alignment and another check. Exit code 2 means the audit did not complete and must never be reported as a pass.

Do not turn preferences, experiments, one-off code, or hypothetical designs into
ways. A way must point to a proven repository-local implementation.

Use standalone `dotnet tool run skies rtw check` only for focused investigation
or maintenance. Do not report work ready until the applicable
`dotnet tool run skies check` exits with code 0.
