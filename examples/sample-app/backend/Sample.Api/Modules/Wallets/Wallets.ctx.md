# Wallets — module context

## Boundaries

Owns a `Wallet` and its running `Balance`. Four slices: `GetBalance` (read), `ListWallets` (the canonical
paginated list — ordered by a unique key, paged with `ToPageAsync` into the framework's `Page` shape),
`Deposit` (the only inflow that grows a balance) and `Withdraw` (the only outflow). Persistence is the
shared `AppDb` — slices talk to
it directly, no repository or unit-of-work layer. The module is a logical bounded context: it writes only
its own `Wallet`, so it stays liftable later even though it shares the one store. The entity and its slices
live here, never in a root `Domain/` folder.

## Design notes

- **`Money` owns the amount rule, once.** "amount >= 0" lives in the `Money` value object; slices only
  `Collect` its failure and `AppDb` only re-checks at the DB↔domain boundary (the converter's
  `Money.From(d).Value`). The rule is never restated in a slice or in `OnModelCreating` (`0004-money#FM-3`;
  a negative deposit is refused with the balance untouched, `0001-deposit#FM-2`).
- **`Wallet` is a rich entity, not a row.** It has no public setter and is opened through `Wallet.Open`;
  the balance moves only through `Deposit` / `Withdraw`. `Withdraw` owns the overdraw invariant — it
  returns a business-rule failure (422) rather than let the balance go negative — so no slice can bypass
  it (`0002-withdraw#FM-2`). Every create and mutate path returns through the private `EnsureValid` funnel,
  so a `Wallet` is never observed or persisted in a broken state.
- **Balance is authoritative server-side.** `Deposit` and `Withdraw` recompute from the stored value and
  never trust a client-sent total (`0001-deposit#FM-1`, `0002-withdraw#FM-1`, `0003-wallet-reads#FM-1`).
- **`Deposit` and `Withdraw` honor an `Idempotency-Key`**: a retry replays the recorded outcome through
  `IIdempotencyStore` instead of applying the write twice (`0001-deposit#FM-5`, `0002-withdraw#FM-5`).
- **`ListWallets` orders by the unique id before paging**, so consecutive pages never overlap or skip a
  wallet (`0003-wallet-reads#FM-5`), and the server clamps hostile paging input before it reaches the query
  (`0003-wallet-reads#FM-4`).
- **`WalletsDb.OnModelCreating` carries storage facts only** (precision 18,2; the `Money`↔decimal
  converter), never domain invariants.
