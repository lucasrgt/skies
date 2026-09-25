# account

User identity: register (email + password), sign in (issue a session), the base profile and role.
Every user belongs to one org (tenancy); registration opens it.
Registration grants no role: `Admin` is the app operator's, assigned out of band.

## Boundaries

- **Inside**: identity — registration, sign-in, the refresh-session lifecycle, the base profile and role.
- **Outside**: anything past *who a user is* lives in its own module — payments, richer profiles,
  notifications, moderation. Account knows the user, not their activity.

## Wiring

- The auth **mechanism** is the `Skies.Framework.Auth` package, registered by the one `AddSkiesAuth` call in
  `AccountModule`: token minting and validation, `IPasswordHasher`, `RefreshSessions` (over this module's
  `UserSessionStore`), and `RefreshCookie`. A security fix there reaches this app through a package version, so
  nothing below re-implements it; see "Mechanism and policy".
- `org` provides tenancy: every `ITenantScoped` entity carries an `OrgId` and is scoped to it (auth
  artifacts like `UserSession` are the deliberate exception — see the auth-bootstrap note). The mechanics are
  `Skies.Framework.EntityFrameworkCore`'s; the resolution (`Tenancy/RequestTenant`) and `Org` are this app's.
- The authenticated caller is `ICurrentUser`, resolved from the access-token claims.
- `AppPolicies.AppAdmin` (registered here) guards writes to app-wide data anywhere in the app.
- `CredentialRateLimit` throttles the `credentials` route group; `Platform.UsePlatform` runs the rate limiter.

## Design notes

Each invariant below cites the failure mode of `0001-auth` that proves it; revise the note and its citation
together.

### Tenancy is invisible to slices
`AppDb` calls `model.ApplyTenantFilters(this)`, a query filter on every `ITenantScoped` entity, and registers the
`TenantStamping` interceptor, which stamps `OrgId` on insert for every save, synchronous or not, and refuses a
write to another org's row: an insert naming another org, or an update or delete of a row stored under one
(`0001-auth#FM-26`). So a normal slice
writes `db.Things.Where(...)` with no org plumbing — it is already scoped to the caller's org. Forgetting the filter
would be a cross-tenant leak, so it is applied to *all* marked entities, not per-entity
(`0001-auth#FM-24`). The one exception is auth-bootstrap — see below.

### Auth-bootstrap and the tenant filter
Register / login / refresh / logout run *before or around* authentication: the request has no org of
its own (login is anonymous; refresh & logout happen once the access token has expired). `RequestTenant` resolves
such a request to **no org**, never a default one, so it reads no org's rows and cannot store a row without naming its
org (`0001-auth#FM-25`). The identity itself — the email, the refresh-token hash — is what
*establishes* the org; it cannot presuppose it. Three consequences:

- **Registration opens the org.** `Register` opens an `Org` and creates the user in it by name
  (`User.Register(org.Id, …)`), so each sign-up starts in an org of its own (`0001-auth#FM-6`).
  Joining an existing org is an invitation flow this app adds when it needs one.
- **The user is looked up across the filter** (`db.Users.IgnoreQueryFilters()`), and the org is derived
  **from the found user**, then put into the JWT. This forced the identity model: **email is unique
  globally** (index on `Email`, not `(OrgId, Email)`), one-human-one-account — a user belongs to one
  org but signs in by email across all of them (`0001-auth#FM-11`).
- **`UserSession` is *not* `ITenantScoped` at all.** It is a global auth artifact keyed by an
  unguessable token hash, never part of an org's dataset — so it is looked up directly, no filter to
  bypass. The tell: an entity you would *always* `IgnoreQueryFilters()` should not be scoped.

Outside auth-bootstrap, never reach for `IgnoreQueryFilters()` on a scoped entity; it is a tenant-safety
bypass and every use is a deliberate, documented exception, not a convenience. The doctor flags each one in module
code, and the auth-bootstrap lookups carry their reason on the suppression beside the call. Work that must span orgs
(a cleanup job) runs in a context of its own over `FixedTenant.System`, never in the request's.

### Access token = stateless JWT
Login issues a 15-minute JWT carrying `sub` (userId), `org`, `role`, and `sid` (the session — see
below). `ICurrentUser` reads those claims — no session lookup per request. For an authenticated
request the org comes from the JWT (`RequestTenant`), so the tenant and the caller can never disagree.

### JWT claim mapping
JwtBearer is configured with `MapInboundClaims = false`. The default remaps `sub` to
`ClaimTypes.NameIdentifier`, which makes `FindFirst("sub")` null and breaks `ICurrentUser`. The profile cases in the auth spec guard this mapping.

### Mechanism and policy
The split is deliberate: what an edit could silently weaken lives in the package; what the product decides lives
here, in plain sight.

- **The package (mechanism):** hashing and verifying passwords and codes (argon2id, constant-time, the dummy
  verification that equalizes timing), minting and hashing opaque tokens, refresh rotation with reuse detection and
  the family burn, the sliding lifetime and absolute ceiling, revocation, the refresh cookie's flags and delivery,
  single use, expiry, and the attempt cap of verification secrets.
- **This module (policy and domain):** the entities and their tables (`User`, `UserSession`, the stores that adapt
  them), the password rule (minimum length), the role model and tenancy, which error code each refusal maps to, who
  may revoke what (`RevokeSession` checks ownership before the package burns the family), each secret's lifetime and
  message, and every endpoint's auth posture.

### Password vs session-token hashing
A **password** is low-entropy → argon2id through `IPasswordHasher`: slow, salted, verified, never reversible. A
**session/refresh token** is high-entropy random → SHA-256: fast and deterministic so it can be looked up by hash.
Hashing a token with argon2 (random salt) would make it impossible to look up. A wrong password never signs in
(`0001-auth#FM-8`). `User` stores the hash as the package's opaque `PasswordHash`; to move to
another algorithm, register a different `IPasswordHasher` after `AddSkiesAuth` and no slice changes.

### Multistep registration
Register lands a user at `EmailPending`. Login does **not** block on an incomplete step — it returns
the step so the client routes to what's next.

### Refresh rotation with theft detection
Login mints a 14-day refresh token and opens a **family** (`UserSession.FamilyId`) through `RefreshSessions`.
`Refresh` spends the presented slot (`UsedAt`) and adds a new slot to the same family — the old token is dead after
one use (`0001-auth#FM-14`).
If a *spent* slot is presented again (`UsedAt != null`), that token leaked: the legit client still holds
the live one, so a second use of a rotated one is **theft**. The package burns the whole family — thief's and
victim's tokens alike — forcing a fresh login, and `Refresh` answers `SessionRevoked`
(`0001-auth#FM-15`). `Logout` revokes the family too (`0001-auth#FM-18`).
(Tokens are Base64Url so they are cookie/URL-safe; the chain grows append-only — pruning spent/expired rows
is a future job.)

There is no time-based replay grace: once rotation commits, presenting the spent token burns the family
immediately. A genuinely simultaneous refresh is distinguished structurally instead: `UserSession.RowVersion`
guards the rotation save with optimistic concurrency. The racer that loses the write gets a
`DbUpdateConcurrencyException`, caught and mapped to `SessionRetry`; the winner already delivered the live
replacement. Thus concurrent use yields one winner and one retry, while every replay observed after rotation
is theft. (`UserSessionStore` turns the exception into that answer; only a relational provider raises it, the
in-memory store never does.)

A family also has an **absolute ceiling** (`RefreshSessionOptions`, 90 days by default): a session may slide
(rotate) freely within that window, but the package retires the family once its first token is older than
the ceiling — so a silently-rotating session cannot live forever, no matter how often it refreshes
(`0001-auth#FM-17`). Both lifetimes are this app's policy, set on `AddSkiesAuth`.

### Security posture (the deliberate choices)
- **No user enumeration by timing.** `Login` hands the hasher the found user's hash or none at all; with none,
  `IPasswordHasher` verifies against a fixed dummy, so a missing email costs the same work as a wrong one, and the
  two return the *same* error. The absence of an account is observable through neither the response nor its timing
  (`0001-auth#FM-9`).
- **A credential change ends every session.** When a password reset (the email flow) or any future
  password change succeeds, it revokes **all** of the user's refresh families through `RefreshSessions`, not
  just the caller's — so a takeover recovery also evicts the attacker.
- **OTP codes are brute-force-capped.** When the phone flow is added, the package's `VerificationTokens` locks a
  6-digit code after five wrong guesses (consuming it), so the 10⁶ space cannot be walked.
- **Registration does not disclose who has an account either.** A taken email gets the answer a new one gets,
  after the same hashing work, and creates nothing; the address's owner is told through `IAccountNotices` (a mail
  once `auth:email` is added, nothing before). The first account is never damaged by the attempt
  (`0001-auth#FM-3`). Google sign-up is the exception: its caller has proven it owns the address.
- **Passwords are 8 to 128 characters.** The ceiling is the hasher's (`IPasswordHasher.MaxPasswordLength`): a
  longer one is refused before any slow hash runs, so one request cannot buy an unbounded derivation
  (`0001-auth#FM-4`).
- **The credential endpoints are throttled.** `CredentialRateLimit` gives each client address a fixed window per
  endpoint on the `credentials` group (register, login, refresh, logout, and every verification or reset endpoint a
  flow adds), answering `429` with `platform.rate_limited`. It is the route group's policy, not slice shape
  (`0001-auth#FM-10`).
- **No one grants themselves Admin.** `AppPolicies.AppAdmin`, the policy on writes to app-wide data, needs the
  `Admin` role, and no endpoint here sets a role (`0001-auth#FM-13`).

### The session id (sid)
A "session" is a refresh family. The access token carries that family id as the `sid` claim, so a
request can name *its own* session without a DB lookup. That is what lets `ListMySessions` flag the
current one (`FamilyId == current.SessionId`), `RevokeSession` refuse to drop a family that isn't the
caller's, and `RevokeOtherSessions` keep the current and burn the rest. Without `sid` the stateless
token could not tell "this session" from the others (`0001-auth#FM-20`, `0001-auth#FM-22`,
`0001-auth#FM-23`). The session row is keyed by `UserId` +
`FamilyId`, never tenant-scoped (it is auth plumbing, not org data).

### Token delivery
Both the access token and the refresh token ride in the response body; every client keeps the refresh
in secure storage and replays it on `/account/refresh` and `/account/logout` in the request body.

## Flows added by generators
`skies g auth:otp`, `auth:oauth`, and `auth:email` add phone verification, Google sign-in, and email verification with
password reset; each brings its provider port and its own spec.
