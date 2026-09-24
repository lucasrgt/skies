---
id: "0006"
runner: web
touches: [frontend/core/src/deposit/Deposit.view.tsx, frontend/core/src/deposit/deposit.i18n.ts]
---
# The Deposit screen

The Deposit form collects a wallet id and an amount, validates them against the Deposit slice's own rules before
anything is sent, announces the submit while it is in flight, and replaces itself with a success surface. The
ViewModel owns the form and the mutation; the View only renders what it hands over.

## Failure modes

- FM-1 An invalid submit reaches the wire, or its field errors are not announced inside the Field anatomy.
- FM-2 A valid submit is not announced as busy while it is pending, or never reaches the success surface.
- FM-3 A failed command is not surfaced as a `role=alert` block, or the form is lost along with it.

## Out of scope

The backend's own validation and the balance it returns: spec 0001 proves the Deposit slice.
