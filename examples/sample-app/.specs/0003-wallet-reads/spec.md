---
id: "0003"
runner: api
touches: [backend/Sample.Api/Modules/Wallets/Wallet.cs]
---
# Read wallets

`GET /wallets/{id}/balance` returns the stored balance. `GET /wallets` lists wallets one page at a time, ordered
by id, with the page size capped by the server.

## Failure modes

- FM-1 The balance read does not reflect the stored balance.
- FM-2 A balance read for a wallet that does not exist answers anything other than 404.
- FM-3 A page does not carry its slice of items together with the count of the whole set.
- FM-4 Hostile paging input (negative page, huge page size) reaches the query instead of being clamped.
- FM-5 Two consecutive pages overlap or skip a wallet.
