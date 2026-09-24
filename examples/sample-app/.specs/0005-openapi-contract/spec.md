---
id: "0005"
runner: api
---
# The OpenAPI contract the clients generate from

`GET /openapi/v1.json` serves the contract the typed React and Flutter clients are generated from. `AddSkies()`
shapes it: every `*ErrorCodes` constant the app owns is enumerated on `ErrorBody.code`, numbers are plain numbers,
scalar value objects mirror their primitive, and the canonical page is required, numeric, and qualified by the
slice that nests its item. A client generated from a contract that drifts on any of these compiles and is wrong.

## Failure modes

- FM-1 `ErrorBody.code` does not enumerate the app's error codes (the Wallets and Money registries, the platform's
  rate limit), so the client cannot type or translate the closed set.
- FM-2 Error codes from a dependency that is merely loaded in the process widen the contract, or an application
  assembly registered on purpose is left out or enumerated twice.
- FM-3 A numeric body property is published as a number-or-string union or with a string pattern.
- FM-4 A scalar value object reached only through a nullable property (`WalletView.LastDeposit`) is published as an
  object instead of its primitive.
- FM-5 A numeric query parameter is published as accepting strings.
- FM-6 The page's four members are optional, or not plain array and integer types, so the spine's structural
  `Page<T>` no longer matches.
- FM-7 A slice's output does not reference its own slice-qualified page schema, so two slices nesting a same-named
  item view collide onto one page.

## Out of scope

How each rule is implemented: the transformers live in `Skies.Framework.AspNetCore`. This spec pins what the
sample's clients consume; the red patch swaps `AddSkies()` for the bare `AddOpenApi()` an app would otherwise write.
