# account

User identity: register (email + password), sign in (issue a session), the base profile and role.
Role is optional at registration — chosen later, after sign-in.

## Boundaries

- **Inside**: identity — registration, sign-in, the refresh-session lifecycle, the base profile and role.
- **Outside**: anything past *who a user is* lives in its own module — payments, richer profiles,
  notifications, moderation. Account knows the user, not their activity.

## Wiring

- The auth **mechanism** is the `Skies.Framework.Auth` package, registered by the one `AddSkiesAuth` call in
  `AccountSetup`: token minting and validation, `IPasswordHasher`, `RefreshSessions` (over this module's
  `UserSessionStore`), and `RefreshCookie`. A security fix there reaches this app through a package version, so
  nothing below re-implements it; see "Mechanism and policy".
- The authenticated caller is `ICurrentUser`, resolved from the access-token claims.

## Design notes

Each invariant below cites the failure mode of `0001-auth` that proves it; revise the note and its citation
together.


### Access token = stateless JWT
Login issues a 15-minute JWT carrying `sub` (userId), `role`, and `sid` (the session — see below).
`ICurrentUser` reads those claims — no session lookup per request.

### JWT claim mapping
JwtBearer is configured with `MapInboundClaims = false`. The default remaps `sub` to
`ClaimTypes.NameIdentifier`, which makes `FindFirst("sub")` null and breaks `ICurrentUser`. The
integration test in `AuthFlow.Tests` guards against a regression here.

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
(`0001-auth#FM-5`). `User` stores the hash as the package's opaque `PasswordHash`; to move to
another algorithm, register a different `IPasswordHasher` after `AddSkiesAuth` and no slice changes.

### Multistep registration
Register lands a user at `EmailPending`. Login does **not** block on an incomplete step — it returns
the step so the client routes to what's next.

### Refresh rotation with theft detection
Login mints a 14-day refresh token and opens a **family** (`UserSession.FamilyId`) through `RefreshSessions`.
`Refresh` spends the presented slot (`UsedAt`) and adds a new slot to the same family — the old token is dead after
one use (`0001-auth#FM-8`).
If a *spent* slot is presented again (`UsedAt != null`), that token leaked: the legit client still holds
the live one, so a second use of a rotated one is **theft**. The package burns the whole family — thief's and
victim's tokens alike — forcing a fresh login, and `Refresh` answers `SessionRevoked`
(`0001-auth#FM-9`). `Logout` revokes the family too (`0001-auth#FM-12`).
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
(`0001-auth#FM-11`). Both lifetimes are this app's policy, set on `AddSkiesAuth`.

### Security posture (the deliberate choices)
- **No user enumeration by timing.** `Login` hands the hasher the found user's hash or none at all; with none,
  `IPasswordHasher` verifies against a fixed dummy, so a missing email costs the same work as a wrong one, and the
  two return the *same* error. The absence of an account is observable through neither the response nor its timing
  (`0001-auth#FM-6`).
- **A credential change ends every session.** When a password reset (the email flow) or any future
  password change succeeds, it revokes **all** of the user's refresh families through `RefreshSessions`, not
  just the caller's — so a takeover recovery also evicts the attacker.
- **OTP codes are brute-force-capped.** When the phone flow is added, the package's `VerificationTokens` locks a
  6-digit code after five wrong guesses (consuming it), so the 10⁶ space cannot be walked.
- **Registration accepts email-existence disclosure.** A duplicate registration returns `409 Conflict`
  (an explicit `EmailTaken`) rather than masking it — a conscious UX trade-off; the mitigation for
  enumeration/abuse is edge rate-limiting, not a vague error. The first account is never damaged by the
  attempt (`0001-auth#FM-3`).
- **Rate limiting is platform infra, not slice shape.** Brute-force / spam protection on the auth
  endpoints (login, register, refresh, password-reset request, OTP) is ASP.NET rate-limiter middleware
  wired in `Platform/Security`, surfacing `platform.rate_limited` — it does not belong in a slice's
  `Handle`. See the platform/security guidance in `docs/CONVENTIONS.md`.

### The session id (sid)
A "session" is a refresh family. The access token carries that family id as the `sid` claim, so a
request can name *its own* session without a DB lookup. That is what lets `ListMySessions` flag the
current one (`FamilyId == current.SessionId`), `RevokeSession` refuse to drop a family that isn't the
caller's, and `RevokeOtherSessions` keep the current and burn the rest. Without `sid` the stateless
token could not tell "this session" from the others (`0001-auth#FM-14`, `0001-auth#FM-16`,
`0001-auth#FM-17`). The session row is keyed by `UserId` +
`FamilyId`, never tenant-scoped (it is auth plumbing, not org data).

### Web vs mobile token delivery
Same endpoints serve both clients; the request opts in with `X-Client: web`. **Web** gets the refresh
token in an httpOnly, Secure, SameSite=Strict cookie scoped to `/account` — invisible to JS, so XSS
can't exfiltrate it — and never in the body; `Refresh`/`Logout` read it back from the cookie. **Mobile/
API** gets the refresh in the body and keeps it in secure storage. The access token always rides in the
body for the `Authorization` header. The route's `Respond` helper keeps the failure path and picks the two
bodies (delivery, not logic — the route stays an expression, `Handle` stays pure and host-free); the framework's
`RefreshCookie` decides which body leaves and plants the cookie (httpOnly/Secure/SameSite are its opinion; the app
sets only the cookie name and path on `AddSkiesAuth`, and the cookie lives exactly as long as the session). A web
client never sees the refresh token in a body (`0001-auth#FM-18`,
`0001-auth#FM-19`).

## Not yet ported
Email verification, phone OTP, OAuth, password reset. The first three need providers (SMTP / SMS / OAuth).
