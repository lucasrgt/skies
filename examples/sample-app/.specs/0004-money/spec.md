---
id: "0004"
runner: api
---
# Money, the always-valid amount

`Money` is the sample's value object for a monetary amount: `Money.From` is the only way to build one, and it
refuses a negative amount, so every `Money` in the system is already valid. It is an isolated system, so its cases
call the type directly instead of booting the app.

## Failure modes

- FM-1 Zero is rejected, although an empty wallet holds exactly zero (the boundary a `>= 0` flipped to `> 0` breaks).
- FM-2 A positive amount is rejected, or comes back as a different amount.
- FM-3 A negative amount is accepted, or rejected as something other than a validation error.
- FM-4 Adding two amounts does not produce their sum.

## Out of scope

Subtraction and overdraw: the `Wallet` entity owns that rule and spec 0002 proves it through the API. The JSON
shape of `Money` on the wire belongs to spec 0005.
