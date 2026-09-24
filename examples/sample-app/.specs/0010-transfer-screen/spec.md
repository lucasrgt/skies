---
id: "0010"
runner: web
touches: [frontend/web/src/transfer/**]
---
# The Transfer screen

The Transfer form collects a source wallet, a destination wallet and an amount, validates them against the Transfer
slice's own rules before anything is sent, announces the submit while it is in flight, and replaces itself with a
success surface. The ViewModel owns the form and the mutation; the View only renders what it hands over. It has the
Deposit screen's shape (`0006-deposit-screen`).

## Failure modes

- FM-1 An empty submit reaches the wire, or its three field errors are not announced inside the Field anatomy.
- FM-2 A submit naming the same wallet on both sides reaches the wire instead of reporting a destination field error.
- FM-3 A valid submit is not announced as busy while it is pending, or never reaches the success surface.
- FM-4 A refused transfer (the server answers 422, insufficient funds) is not surfaced as a `role=alert` block, or the form is lost with it.

## Out of scope

- The backend's validation and the balances it computes: spec `0009-transfer` proves the Transfer slice.
