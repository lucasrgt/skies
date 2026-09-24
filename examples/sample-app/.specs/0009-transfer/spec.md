---
id: "0009"
runner: api
---
# Transfer money between two wallets

`POST /wallets/transfer` moves an amount from one wallet to another and returns both new balances. The debit and the
credit are one change: either both land or neither does, so money is never created or lost in transit. The source's
overdraw rule is the one `Withdraw` enforces, and a request carrying an `Idempotency-Key` is applied at most once.

## Failure modes

- FM-1 A valid transfer does not debit the source and credit the destination by the amount, as later balance reads see it.
- FM-2 A transfer that would overdraw the source is accepted, answers something other than 422, or moves either balance.
- FM-3 A transfer from a wallet to itself is accepted, answers something other than 422, or moves the balance.
- FM-4 A transfer whose source or destination does not exist answers anything other than a 404 carrying `wallets.not_found`, or moves the other wallet.
- FM-5 A request with an empty source, an empty destination and a negative amount does not report all three field errors at once.
- FM-6 A retry carrying the same `Idempotency-Key` moves the money a second time. [avp: idempotency-key-honored]

## Out of scope

- Caller identity and wallet ownership: the sample ships no identity provider (as in `0001-deposit`).
- Concurrent transfers racing on the same wallet: the `RowVersion` token surfaces the conflict on a relational store,
  but the in-memory test store does not enforce it, so this spec cannot prove it.
- Currency: the sample's `Money` carries none.
