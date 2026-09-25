# Skies — Auth: the mechanism is a package, the policy is the app's

Authentication is the feature most worth proving and least worth rewriting per app, so Skies splits it: the
security mechanics ship as `Skies.Framework.Auth`, and the generated Account module holds the app's policy and data
in plain slices. The backend conventions are [CONVENTIONS.md](CONVENTIONS.md).

Generated Account modules use `[Module]`, `AddServices`, and `Map` through the same registry as other modules.
`Platform.AddPlatform` owns the database and external providers. The starter's InMemory database, local JWT key,
and fake/console providers live together inside its Development branch: startup refuses other environments until the
owner replaces that branch with persistent storage, a configured `Jwt:Secret`, and real providers. Replacing the
branch takes the development key with it, and the package refuses the key anyway (see
[The signing secret](#the-signing-secret)). For OAuth use `OidcIdTokenVerifier` with the provider authority and client
id. Keep local substitutes inside an explicit Development branch. Generation refuses to overwrite existing owner files
before writing anything.

`skies g auth` (and `auth:otp`, `auth:oauth`, `auth:email`) generates the Account module's slices, entities, and
spec, but not the security mechanics: those are `Skies.Framework.Auth` (and the `Identity` port), so a fix reaches
every app through a package version instead of a template no generated app ever sees again. The module registers
them with one explicit call in its composition, `AddSkiesAuth<UserSessionStore>(new SkiesAuthOptions(...))` (plus
`AddVerificationTokens<VerificationTokenStore>()` once a phone or email flow is added).

| Mechanism (package) | Policy and domain (app) |
|---------------------|-------------------------|
| `IPasswordHasher` (argon2id): hash, constant-time verify, the dummy verification that makes "no such account" cost what "wrong password" costs, and the 128-character ceiling (`MaxPasswordLength`) enforced before any derivation | the password rule (minimum length, and reporting the ceiling as a validation error) and when a password may change |
| `OpaqueTokens`: 256-bit tokens, SHA256 lookup hashes, constant-time `Matches` | nothing: a token is never app-shaped |
| `RefreshSessions`: open a family, rotate, burn the family on reuse, the sliding lifetime and absolute ceiling, revoke by token, family, user, or all-but-current | which error code each `RefreshOutcome` maps to, who may revoke which session (the ownership check), the two lifetimes (`RefreshSessionOptions`) |
| `VerificationTokens`: issue and consume single-use links and 6-digit codes, expiry, purpose binding, the attempt cap that locks a code | each purpose's lifetime, the message that carries the secret, what a verified secret unlocks, the cap (`VerificationOptions`) |
| `RefreshCookie`: httpOnly/Secure/SameSite, the `X-Client: web` switch, `Deliver` (which body leaves, cookie lifetime = session lifetime) | the cookie's name, path, and (for a multi-subdomain app) domain and SameSite; the two response bodies |
| `IAccessTokens`, `ICurrentUser`, the JwtBearer validator, from one secret/issuer/audience | the claims' values: org, role, display name |
| `OidcIdTokenVerifier` (`IExternalIdentityVerifier`): signature against the provider's keys, issuer, audience, lifetime, verified email | which providers the app trusts and their client ids |
| Tenancy (`Skies.Framework.EntityFrameworkCore`): `ITenantScoped`, the `ITenant` seam, `ApplyTenantFilters` (reads), the `TenantStamping` interceptor (every save, sync or async) | how a request resolves its org (`RequestTenant`), the `Org` entity, and when registration opens one |

- **Persistence stays app-owned.** The app keeps `User`, `UserSession`, `VerificationToken`, `Org`, and its role
  model as ordinary `[Entity]` types in its own `AppDb`. The package reaches them only through two small store
  interfaces the app implements in a few lines of EF each (`IRefreshSessionStore`, `IVerificationStore`): plain data
  access, no decisions. The services re-check whatever a store returns (hash in constant time, purpose, expiry, use),
  so a loose store can make a secret fail, never pass.
- **Slices keep the Skies shape.** `Input`/`Output`/`Handle`/`Map`, the error codes, and the auth posture stay in the
  slice; `Handle` takes the service it needs (`IPasswordHasher`, `RefreshSessions`, `VerificationTokens`) and maps
  each outcome enum to its `AccountErrorCodes` constant. No base class, no generated code, no discovery: the services
  are registered by name and injected like any other.
- **Moving an app generated before this.** `skies migrate 5` never rewrites auth code, which is the app's own; it
  notes a Skies 4 Account module. Regenerate with `skies g auth` in a branch, compare, and port the app's changes.

## Tenancy: one org per registration, none for an anonymous request

The generic mechanics ship in `Skies.Framework.EntityFrameworkCore` (the EF satellite, so the runtime packages stay
persistence-agnostic); the resolution and the `Org` entity are the app's. `AppDb` implements `ITenantDbContext`
(`CurrentOrgId` reads the request's `ITenant`), calls `model.ApplyTenantFilters(this)` last in `OnModelCreating`, and
adds the `TenantStamping` interceptor in `OnConfiguring`.

- **Reads.** One named query filter (`tenant`) on every `ITenantScoped` root entity. It is the one model scan Skies
  keeps: it walks the entities the context registered (never assemblies), because a per-entity opt-in fails open, and
  a forgotten filter is a cross-tenant leak.
- **Writes.** An interceptor, so the synchronous `SaveChanges` checks exactly like `SaveChangesAsync`. A unit of work
  acting in an org (a signed-in request, a `FixedTenant` for a known org) writes only that org's rows: an insert
  without an org takes the context's, an insert naming the same org keeps it, and an insert naming another org is
  refused; an update or delete is refused unless the row's stored org (its original value, so a detached row
  attached by hand with `Remove` or `Update` is caught too) is the context's. In every scope a row with no org to take,
  or one moved to another org, is refused. Each refusal is an `InvalidOperationException` before anything is stored:
  a cross-org write is a bug in the slice, not a user error.
- **The system scope, the one escape.** A unit of work with no org (`FixedTenant.System`, or an anonymous request)
  reads no tenant-scoped row through the filter, so the only rows it can change are ones it loaded on purpose across
  the filter or attached by hand, and it can insert only rows that name their org. That is how registration stores a
  user in the org it has just opened, how the anonymous auth-bootstrap slices (reset, verification) change a user,
  and how a job that spans orgs (a cleanup, a data migration) runs: in a context of its own, created over
  `FixedTenant.System` in its own DI scope, never by lifting the filter inside a request.
- **Cross-org reads are announced.** `IgnoreQueryFilters()` in module code lifts the tenant filter, so the doctor
  flags it (`SKY0030`) unless it lifts only a named non-tenant filter. A deliberate crossing takes
  `#pragma warning disable SKY0030 // <reason>` beside the call, and the reason is required. The generated Account
  module's auth-bootstrap lookups carry theirs.
- **Resolution.** `RequestTenant`: a signed-in request acts in its access token's `org`; an anonymous request
  resolves **no org** (`Guid.Empty`), so it reads no tenant-scoped row and cannot store one without naming its org.
  There is no default org. A signed-out page that must show one org's data (a storefront on its own subdomain)
  resolves that org in `RequestTenant`, explicitly, from the host.
- **Registration opens the org.** `Register` (and `RegisterWithGoogle`) opens an `Org` and creates the user in it by
  name, so every sign-up starts alone. Joining an existing org is an invitation flow the app adds; the blueprint
  ships none. Sign-in, uniqueness, and the auth-bootstrap lookups cross the filter (`IgnoreQueryFilters`) because
  identity is global and precedes the org. A registration that carries a valid access token is refused with `409
  account.already_signed_in`: the new account's org is not the caller's, so the client signs out first.

## Real-time hubs: rooms are the org's

`skies g hub <Module> <Name>` generates an `[Authorize]` SignalR hub whose rooms are scoped by the caller's org. The
group a room key names is derived on the server from the org claim of the caller's access token plus the key
(`org:<org>/room:<key>`), so two orgs using the same key ("general") never share a room and no client can name
another org's group. `JoinRoom` checks the key and the hub's participation rule (`MayEnter`: any signed-in member of
the org by default; narrow it to the module's rule, such as "is in the conversation", before shipping a room that is
not org-wide). `Broadcast` sends only to a room the connection has joined, and only to the caller's org's group. A
single-tenant app's tokens carry no org, so every signed-in user shares one room per key. The hub is wired in its
module (`services.AddSignalR()` in `AddServices`, `app.MapHub<…>` in `Map`), as the generator prints.

## App-wide data and the admin policy

An entity that is not `ITenantScoped` is app-wide: every org reads the same rows. `g auth` defines
`AppPolicies.AppAdmin` (`AppPolicies.cs` at the API root), registered in `AccountModule` as "has the `Admin` role".
Registration never grants a role and no generated endpoint sets one: the operator assigns `Admin` out of band. `g
crud` maps an app-wide entity's reads under the module group and its Create/Update/Delete under a second group of the
same prefix that requires `AppPolicies.AppAdmin`. Without the auth blueprint that group requires a policy no one
satisfies, so app-wide writes are closed until the owner names who may make them. Tenant-scoped entities keep all six
slices under the module group: the org already scopes them.

## Threat model in short

- **Enumeration.** No endpoint says whether an email has an account. Login answers a missing account and a wrong
  password identically, after the same argon2 work. Register answers a taken email exactly as a new one (the same
  empty `200`, the same hashing), creates nothing, and tells the address's owner through `IAccountNotices`: a mail
  once `g auth:email` is added (`EmailAccountNotices`), nothing before it, since there is no channel. The trade-off
  without email: someone who re-registers their own address is told "done" and must sign in with the old password.
  Password reset answers the same whatever the address. `RegisterWithGoogle` is the one `409`: its caller has proven
  it owns the address. Register still takes longer when it mails the notice; send mail through a queue in
  production to close that gap too.
- **Guessing and flooding.** `CredentialRateLimit` (ASP.NET Core's built-in rate limiter, the named policy
  `account-credentials` on the Account module's `credentials` route group) gives each client IP address
  (`HttpContext.Connection.RemoteIpAddress`) a fixed window per endpoint: 10 requests a minute by default, configured by `Account:RateLimit:PermitLimit` and
  `Account:RateLimit:WindowSeconds`. It covers register, login, refresh, logout, and every flow endpoint (reset,
  verification, phone codes, Google sign-in). A throttled request answers `429` with `platform.rate_limited` and a
  `Retry-After` header; `Platform.UsePlatform` runs `app.UseRateLimiter()`. The throttle is per address, not per account:
  lockout per account is the app's to add if it wants one. Six-digit codes also lock after five wrong guesses.
  Behind a reverse proxy or load balancer the connection's address is the proxy's, so every client shares one
  budget (and one throttled client locks everyone out) until `UseForwardedHeaders` runs first in
  `Platform.UsePlatform`, with the proxy listed in `ForwardedHeadersOptions.KnownProxies` (or `KnownNetworks`). Trust
  only your own proxies: a forwarded header from anyone else lets a client pick its own address and slip the limit.
- **Password limits.** 8 to 128 characters. The ceiling (`IPasswordHasher.MaxPasswordLength`) bounds what one
  request can make the slow hash do: `Hash` refuses a longer password and `Verify` fails it after a fixed-size dummy
  derivation, and Register and ResetPassword report it as `password.too_long`.
- **Token lifetimes.** Access tokens (HS256, pinned) live 15 minutes with 30 seconds of clock skew. They are
  stateless: logout, a revoked session, and a password reset end the refresh family, but an access token already
  issued stays valid until it expires, up to 15 minutes (and the skew) later. Shorten that window with
  `SkiesAuthOptions.AccessTokenLifetime` on `AddSkiesAuth` (a few minutes costs only more refreshes); an app that
  needs revocation to bite at once checks the token's `sid` against its session table on each request (a
  `JwtBearerEvents.OnTokenValidated` handler that fails the request when the family is gone), paying a lookup per
  request for it. Refresh tokens
  slide for 14 days inside a 90-day absolute session ceiling, rotate on every use, and burn their family on replay.
  Email links live 24 hours (verification) or 1 hour (reset); phone codes 10 minutes. All single use.
- **Concurrency.** `UserSession` rotation races resolve on the row version (`auth.session_retry`). Generated CRUD
  checks a `Version` token on every update and delete and answers `409` on a stale one.

## Before deploying

The generated platform refuses to start outside Development until these are the app's own:

- A persistent `AppDb` provider in `Platform.AddPlatform` (the starter's is in-memory) and its migrations.
- `Jwt:Secret`: at least 32 random bytes, from a secret store, never the development key (see below).
- Real providers for every flow added: `IEmailSender`, `ISmsSender`, and `OidcIdTokenVerifier` with the provider's
  authority and client id.
- Forwarded headers (`UseForwardedHeaders` with the proxy's address, first in `Platform.UsePlatform`) when the app
  runs behind a proxy or load balancer, or the rate limiter sees one client address and throttles everyone together.
- The rate limit, if 10 a minute per address and endpoint does not fit the traffic (`Account:RateLimit`).
- At least one `Admin`, assigned out of band, if the app has app-wide data to change.
- Each hub's participation rule (`MayEnter`), if a room is narrower than the whole org.
- For the web: serve over HTTPS (the refresh cookie is `Secure`), and set the cookie's `Domain`/`SameSite` only for a
  real multi-subdomain case.

## The signing secret

Access tokens are HMAC-signed with `Jwt:Secret`, so whoever knows the secret can mint a token for any user in any
org. The generated `Platform` sets `SkiesAuthOptions.DevelopmentSecret` only inside its Development branch, so a
fresh clone runs with no setup. `Skies.Framework.Auth` checks the secret when the host starts (`ValidateOnStart`) and
refuses to start:

- anywhere, when the secret is missing;
- outside Development, when it is shorter than 32 bytes (`SkiesAuthOptions.MinSecretBytes`), or is a development
  key (`SkiesAuthOptions.DevelopmentSecret`, or any `<app>-local-development-key-not-for-deployment` key an earlier
  generated app set).

The failure names the setting to configure. Set `Jwt:Secret` from a secret store or the `Jwt__Secret` environment
variable, generated once per environment (`openssl rand -base64 48`), and rotate it by redeploying: every access token
signed with the old secret stops validating, and clients refresh. The build-time OpenAPI pass signs nothing and is not
checked.
