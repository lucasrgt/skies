---
id: "0002"
runner: api
---
# Withdraw money from a wallet

`POST /wallets/withdraw` debits a wallet and returns the new balance. The `Wallet` entity owns the overdraw
invariant, so no caller can take the balance below zero.

## Failure modes

- FM-1 A valid withdrawal does not change what a later balance read returns.
- FM-2 Overdrawing is accepted, answers something other than 422, or moves the balance.
- FM-3 A negative amount is accepted instead of answering 400 with an `amount` field error.
- FM-4 A withdrawal from a wallet that does not exist answers anything other than 404.
- FM-5 A retry carrying the same `Idempotency-Key` debits the wallet a second time. [avp: idempotency-key-honored]
