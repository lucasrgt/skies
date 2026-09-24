# Skies — Auth: the mechanism is a package, the policy is the app's

Authentication is the feature most worth proving and least worth rewriting per app, so Skies splits it: the
security mechanics ship as `Skies.Framework.Auth`, and the generated Account module holds the app's policy and data
in plain slices. The backend conventions are [CONVENTIONS.md](CONVENTIONS.md).

Generated Account modules use `[Module]`, `AddServices`, and `Map` through the same registry as other modules.
`Platform.AddPlatform` owns the database and external providers. The starter's InMemory database, local JWT key,
and fake/console providers are Development-only: startup refuses other environments until the owner replaces
that platform setup with persistent storage, a configured `Jwt:Secret`, and real providers. For OAuth use
`OidcIdTokenVerifier` with the provider authority and client id. Keep local substitutes inside an explicit
Development branch. Generation refuses to overwrite existing owner files before writing anything.

`skies g auth` (and `auth:otp`, `auth:oauth`, `auth:email`) generates the Account module's slices, entities, and
spec, but not the security mechanics: those are `Skies.Framework.Auth` (and the `Identity` port), so a fix reaches
every app through a package version instead of a template no generated app ever sees again. The module registers
them with one explicit call in its composition, `AddSkiesAuth<UserSessionStore>(new SkiesAuthOptions(...))` (plus
`AddVerificationTokens<VerificationTokenStore>()` once a phone or email flow is added).

| Mechanism (package) | Policy and domain (app) |
|---------------------|-------------------------|
| `IPasswordHasher` (argon2id): hash, constant-time verify, the dummy verification that makes "no such account" cost what "wrong password" costs | the password rule (minimum length) and when a password may change |
| `OpaqueTokens`: 256-bit tokens, SHA256 lookup hashes, constant-time `Matches` | nothing: a token is never app-shaped |
| `RefreshSessions`: open a family, rotate, burn the family on reuse, the sliding lifetime and absolute ceiling, revoke by token, family, user, or all-but-current | which error code each `RefreshOutcome` maps to, who may revoke which session (the ownership check), the two lifetimes (`RefreshSessionOptions`) |
| `VerificationTokens`: issue and consume single-use links and 6-digit codes, expiry, purpose binding, the attempt cap that locks a code | each purpose's lifetime, the message that carries the secret, what a verified secret unlocks, the cap (`VerificationOptions`) |
| `RefreshCookie`: httpOnly/Secure/SameSite, the `X-Client: web` switch, `Deliver` (which body leaves, cookie lifetime = session lifetime) | the cookie's name, path, and (for a multi-subdomain app) domain and SameSite; the two response bodies |
| `IAccessTokens`, `ICurrentUser`, the JwtBearer validator, from one secret/issuer/audience | the claims' values: org, role, display name |
| `OidcIdTokenVerifier` (`IExternalIdentityVerifier`): signature against the provider's keys, issuer, audience, lifetime, verified email | which providers the app trusts and their client ids |

- **Persistence stays app-owned.** The app keeps `User`, `UserSession`, `VerificationToken`, its role model, and
  tenancy as ordinary `[Entity]` types in its own `AppDb`. The package reaches them only through two small store
  interfaces the app implements in a few lines of EF each (`IRefreshSessionStore`, `IVerificationStore`): plain data
  access, no decisions. The services re-check whatever a store returns (hash in constant time, purpose, expiry, use),
  so a loose store can make a secret fail, never pass.
- **Slices keep the Skies shape.** `Input`/`Output`/`Handle`/`Map`, the error codes, and the auth posture stay in the
  slice; `Handle` takes the service it needs (`IPasswordHasher`, `RefreshSessions`, `VerificationTokens`) and maps
  each outcome enum to its `AccountErrorCodes` constant. No base class, no generated code, no discovery: the services
  are registered by name and injected like any other.
- **Moving an app generated before this.** `skies migrate 5` never rewrites auth code, which is the app's own; it
  notes a Skies 4 Account module. Regenerate with `skies g auth` in a branch, compare, and port the app's changes.
