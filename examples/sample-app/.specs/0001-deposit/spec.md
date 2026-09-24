---
id: "0001"
runner: api
---
# Deposit money into a wallet

`POST /wallets/deposit` credits a wallet and returns the new balance. It is the only inflow that grows a balance.
The balance is recomputed server-side from the stored value.

## Failure modes

- FM-1 A valid deposit does not change what a later balance read returns.
- FM-2 A negative amount is accepted, or rejected without leaving the balance untouched.
- FM-3 A request with an empty wallet id and a negative amount reports only one of the two field errors.
- FM-4 A deposit into a wallet that does not exist answers anything other than 404.
- FM-5 A retry carrying the same `Idempotency-Key` credits the wallet a second time.
- FM-6 A new `Idempotency-Key` is treated as a replay and does not credit.

## Out of scope

Caller identity and wallet ownership: the sample ships no identity provider.
