# Decision: the doctor must catch the *unmarked* entity / value object, not only police the marked one

**Status:** accepted; implemented (`SKY0021`, `analyzers/Skies.Framework.Doctor/UnmarkedDomainTypeAnalyzer.cs`) + tests
(`tests/Skies.Framework.Doctor.Tests/UnmarkedDomainTypeAnalyzerTests.cs`, 5 cases, green). Self-graded
**8.7 — PASS** (see §Grading).
**Date:** 2026-06-08.
**Supersedes/extends:** `ValueObjectAnalyzer` (SKY0013) + `EntityAnalyzer` (SKY0014). It does not replace
them — it adds the rung *below* them: they grade a type that is already marked; this rule catches the type
that should be marked and isn't.
**Lineage:** the same anti-theater intent as the verification decision
([`skies-framework-fail-closed-verification.md`](skies-framework-fail-closed-verification.md)) — "a green doctor is
not shipping safety" — applied to the **domain-mark** grain. A green SKY0013/SKY0014 says nothing about the
class that simply never opted in.

---

## Context

The pauta port (Go/`.lzi` → .NET) surfaced the hole in a code review. Three observations, one root cause:

1. **`Account/User.cs` is an anemic bag** — `public class User` with `public Guid Id { get; set; }` on every
   field, no `[Entity]`, no factory, no `EnsureValid`. The source `account.User` is a `resource` with a
   `registration_step` lifecycle and invariants — a textbook entity. The sibling `Agency.cs` was ported
   *correctly* ([Entity], `Create` factory, `EnsureValid`, private setters), which makes the gap a port
   omission, not a misunderstanding.
2. **`BuildingBlocks/Address.cs` is a plain record, no `[ValueObject]`** — all-optional, public positional
   ctor, and (the tell) its XML-doc talks about a `Host` that does not exist in pauta. It is residue copied
   from the hostpoint reference port and never wired to anything.
3. **Five `Address` types where the source has one** — `AgencyAddress`, `CustomerAddress`,
   `MediaVehicleAddress`, `SupplierAddress` (each a correct `[ValueObject]`) plus the orphan above. The
   parallel port agents each re-derived a per-module address with no consolidation pass.

The load-bearing fact is **why the doctor stayed green through all three**:

> **SKY0013 and SKY0014 only fire on a type that is *already* marked.** `EntityAnalyzer.cs:55` returns early
> when `!IsEntity(cls)`; `ValueObjectAnalyzer.cs:52` returns early when `!IsValueObject(type)`. An anemic
> class that never says `[Entity]` is not a *broken* entity to them — it is *not an entity at all*, so they
> never look. **The omission of the mark is the evasion.**

This is structurally identical to the journey decision's hole (SKY0008 proves a journey *exists*; it never
read the body). Here the marks are the on-ramp to enforcement, and an author — or a porting agent — can skip
enforcement entirely by skipping the on-ramp. The framework documents a domain discipline
(`CONVENTIONS.md` §"The domain") that its analyzers cannot see a violation of, because the violation is an
absence.

There is a ceiling worth naming up front, because it bounds what this rule can honestly promise:

| Rung | Claim | Statically enforceable? |
|---|---|---|
| 1. Marked-shape | "a type *marked* `[Entity]`/`[ValueObject]` obeys the shape" | ✅ — SKY0013 / SKY0014 today |
| 2. Mark-required (persisted) | "a type that is a **table** is marked `[Entity]`" | ✅ — DbSet is an exact signal (this rule, Detector A) |
| 3. Mark-required (owned) | "a complex member of an entity is a marked value object" | ✅ — *in this architecture* (this rule, Detector B) |
| 4. Mark-required (general) | "any class that *models a domain concept* is marked" | ❌ — undecidable; "domain concept" has no syntax |

Rung 4 is the wall: there is no signal that separates a `PricingPolicy` that should be a value object from a
`PagedResult<T>` that should not, in general. This rule deliberately stops at rungs 2–3, where the signal is
**structural and exact**, and does not guess at rung 4. The cost of overreach here is false positives on
every DTO and options bag — which would train authors to suppress the rule, destroying it.

## Decision

Add **SKY0021 — a persisted or entity-owned type must declare its mark**, with two detectors, each keyed to a
fact that is exact under these conventions (so the rule is `Error`, not a heuristic warning):

### Detector A — DbSet ⟹ `[Entity]`

A type used as `DbSet<T>` on a `DbContext`-derived class is a persisted aggregate root — a table — i.e. an
entity by definition. If `T` is project-owned and not `[Entity]` (and not `[Keyless]`, the read-model
opt-out), flag it. Reported at `T`'s declaration. *This is the precise catch for the `User` bug.*

### Detector B — complex member of an `[Entity]` ⟹ `[ValueObject]`

A property on an `[Entity]` whose type (after stripping one `Nullable<>` and one collection layer) is a
project-owned class/record that is **not** `[ValueObject]`, **not** `[Entity]`, and **not** an `enum` is
entity state with no mark. This is exact *here* because the convention fixes what entity state may be —
value objects + primitives + enums + ids, with cross-module references carried **by id, never an EF
navigation** (`CONVENTIONS.md` §"Project layout", modular-monolith bullet; enforced adjacent by SKY0009). So
a bare complex member is a value object that forgot `[ValueObject]`. Reported at the property. *This is the
precise catch for an `Address`-on-`Agency` that had shipped unmarked.*

### What it deliberately does **not** do

- **It does not flag dead/unused types.** The orphan `BuildingBlocks/Address.cs` is *not* caught — it is
  persisted by nothing and owned by no entity. That is correct: dead scaffolding is a different defect
  (delete it), and asking the doctor to mark unused code would push it toward rung 4. The fix for the orphan
  is deletion + consolidation, tracked in the port, not in this rule.
- **It does not flag types it doesn't own.** A member typed `string`, `Guid`, `DateTime`, or any framework
  type (no source location) is never required to be marked.
- **It does not re-check shape.** Once a type *is* marked, SKY0013/SKY0014 own its shape. SKY0021 is purely the
  "you forgot the mark" rung beneath them.

## New diagnostics

Net-new, none exists today (honest per the rubric's Criterion 8.5):

- `SKY0021` — a `DbSet<T>` whose `T` is unmarked (Detector A), or an `[Entity]` member of an unmarked complex
  type (Detector B). Next free id at the time; SKY0001–SKY0020 were taken.

Existing, cited as-is: `SKY0013` (`ValueObjectAnalyzer.cs:23`), `SKY0014` (`EntityAnalyzer.cs:26`), `SKY0009`
(`ModuleBoundaryAnalyzer` — the by-id, no-navigation rule Detector B leans on).

## Scope / boundary

Generic, in-boundary. Every Skies app has entities and value objects; the marks and the `DbContext` are
framework-conventional. This hardens an existing in-boundary mechanism (the marks) rather than moving the
80/20 line — so the one-pilot pauta incident plus the SKY0013/SKY0014 lineage is sufficient justification. No
provider names, no DI/transport mechanics, no runtime invasion: a pure syntactic+semantic analyzer,
doctor-removable like every other (`CONVENTIONS.md` Law 2).

## What it removes

The false confidence that a clean SKY0013/SKY0014 means "the domain is encapsulated." Before this, the cheapest
way to a green domain was to *not mark the type* — enforcement was opt-in, and the opt-out was silent. After
this, a persisted or owned type cannot stay unmarked: the marks become the only way to model state, and
"unmarked" stops being a hiding place. It adds one diagnostic, zero namespaces, zero authoring surface (you
were already supposed to mark these).

## Grading (self-assessment, cruel-first)

Gate: ≥ 8.5 weighted **and** no criterion < 7 → PASS; any < 6 → BLOCK; boundary violations always BLOCK.

| # | Criterion | Score | Best evidence | Weakest spot |
|---|---|---|---|---|
| 1 | Cold-read legibility of the resulting convention | 9.0 | the rung table draws the exact line; "marks are the on-ramp" frames it in one sentence | an author must learn *why* a DTO is fine but an entity member isn't — answered by the rung table, not obvious without it |
| 2 | Scope / boundary discipline (80/20, no runtime invasion) | 9.5 | reuses the framework's own marks + DbContext; pure analyzer, doctor-removable | Detector B leans on the by-id convention — correct *here*, would mislead a codebase that allows EF navigations (which this framework forbids, so in-boundary) |
| 3 | Determinism — one signal per detector | 9.0 | DbSet (A) and entity-member-type (B) are each a single structural fact, no author fork | "one collection layer / one nullable layer" of unwrap is a bounded choice; a `List<List<Address>>` would slip (no real case) |
| 4 | Doctor enforcement named + severity path | 9.0 | `SKY0021`, `Error`, implemented + green rule tests | one unconditional severity keeps the contract legible |
| 5 | Diagnostic-ID truthfulness (rubric C8.5) | 9.5 | `SKY0021` is genuinely net-new; the id collision check (SKY0020 taken) is in the code comment | none material |
| 6 | Testability | 9.0 | five cases: valid, DbSet-unmarked, member-unmarked, nullable-unwrap, the full allowed-state regression | no test yet for `[Keyless]` opt-out or `[NotMapped]` skip (both coded, both unexercised) |
| 7 | Anti-theater honesty — does it catch the bug it claims? | 8.5 | Detector A catches the exact `User` bug; Detector B the exact `Address`-on-entity bug; the orphan is honestly declared out-of-scope | it does **not** catch the orphan the reviewer first pointed at — defensible (dead code), but must be said plainly, not hidden |
| 8 | Smallness / what it removes | 8.5 | removes "unmarked = invisible"; +1 diagnostic, 0 namespaces, 0 authoring surface | adds a third domain rule — real surface, justified by the evasion it closes |

Weighted ≈ **8.7**. No criterion < 7 → **PASS.**

**Cruel summary.** The rule's honesty hinges on resisting rung 4. Detectors A and B are exact *because they
refuse to ask "is this a domain concept?"* and instead ask "is this a table?" / "is this entity state?" —
both of which have a syntax. The day someone asks SKY0021 to also flag the orphan `Address`, or any
"looks-domainish" class, it becomes a heuristic, earns suppressions, and dies. Keep it at rungs 2–3. The one
soft floor is C6: the `[Keyless]` and `[NotMapped]` opt-outs are coded but untested — add those two cases.

### Tracked cuts (PASS)

- Add test cases for the `[Keyless]` (Detector A) and `[NotMapped]` (Detector B) opt-outs.
- Port-side follow-through (not this rule): finish `User` as `[Entity]`, consolidate the five `Address`
  types to one shared `[ValueObject]`, delete the orphan.
