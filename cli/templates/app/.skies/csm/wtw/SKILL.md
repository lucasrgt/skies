---
name: why-this-way
description: Preserve and retrieve repository decisions and invariants with CSM-managed WTW.
---

# Why This Way

The primary agent retrieves WTW together with every other foundation by running
`dotnet tool run skies context --task "<goal>"` before implementation. Before
commit it stages the intended paths and runs
`dotnet tool run skies check --task "<completed task>" --staged`; the repository
chooses one checked local-or-CI affected authority and exposes an explicit
`--full` release command. Do not assign
a separate permanent agent to WTW.

Use `dotnet tool run skies wtw explain` only when deeper decision inspection is
needed. Read the returned authority, rationale, alternatives, violation
examples, and links before choosing an implementation.

Hosts call `dotnet tool run skies wtw collect` after an authoritative source
contains a durable choice or falsifiable invariant. There is no manual add
command. Two isolated judges must return the same evidence-backed candidate
before it is written.

Use `dotnet tool run skies wtw guard` only for focused investigation or
maintenance. The standard `skies check` receipt already runs it and treats a
malformed record, conflicting local relation, dangling WTW URI, or suite-mode
invariant without an inbound proof as a blocking health failure.
