// The route-guard decision, as PURE data. The read-side twin of the session seam: `toSessionState` projects "who
// is signed in"; this answers "may this route render for them, and if not, where to". It is router-agnostic by the
// same trick as the rest of the spine (cf. BackRouter/QueryLike) — it NEVER navigates, it returns a {@link
// GuardOutcome} the app's thin `<AuthRoute>`/`<GuestRoute>` turns into its router's `<Navigate>` +
// splash. The point of the symmetry: an auth-guard and a guest-guard are the SAME primitive with `allow` flipped,
// so guarding a PUBLIC route (the login/signup screen a signed-in user must be bounced OFF — the pauta bug where a
// logged-in user reaching /login was let through) stops being something each app re-derives by hand.

import type { SessionState } from "./session";

/**
 * What a guard decided, for the app to act on — three cases, mirroring {@link SessionState}'s discipline:
 *
 * - `wait` — the session is still resolving; render a splash, decide nothing (the load-bearing case a boolean guard
 *   collapses, then bounces a not-yet-settled user — see {@link SessionState}).
 * - `render` — the visitor is allowed; render the route.
 * - `redirect` — the visitor is not allowed; send them to `to`.
 *
 * @typeParam Href - the app's route type (a string on every router; generic so typed routes keep their checking).
 */
export type GuardOutcome<Href = string> =
  | { action: "wait" }
  | { action: "render" }
  | { action: "redirect"; to: Href };

/**
 * How a route is guarded. `allow` is the ONLY axis that differs between an auth-guard, a guest-guard, and a
 * capability-guard — one primitive, one flag.
 *
 * @typeParam U - the authenticated user (what a capability predicate inspects).
 * @typeParam Href - the app's route type.
 */
export interface GuardOptions<U = unknown, Href = string> {
  /** Who may render this route:
   * - `"authenticated"` — a private screen (any signed-in user);
   * - `"anonymous"` — a public/guest screen (login, sign-up), which a signed-in user must be redirected away from;
   * - a **predicate over the user** — a capability/role gate (`(u) => u.role === "admin"`). Authorization is a
   *   fact about the user DATA, not a second identity axis (the spine's invariant: auth = one identity,
   *   authz = capability), so a role guard is the same primitive carrying a predicate — never a bespoke,
   *   easy-to-forget hand-rolled check. An anonymous visitor never satisfies it (there is no user), so they
   *   redirect like any rejected visitor. */
  allow: "authenticated" | "anonymous" | ((user: U) => boolean);
  /** Where to send a visitor the guard rejects — the sign-in screen for an auth-guard, the app home for a
   * guest-guard, a "no access" / home screen for a capability-guard. */
  redirectTo: Href;
}

/**
 * Decide a route guard from a {@link SessionState}, as pure data — no navigation, no JSX. Bind it to your router in
 * a ~10-line app component, written ONCE for both directions:
 *
 * ```tsx
 * function Guard({ allow, redirectTo, children }: GuardOptions & { children: ReactNode }) {
 *   const outcome = guardSession(useSessionState(), { allow, redirectTo });
 *   if (outcome.action === "wait") return <Splash />;
 *   if (outcome.action === "redirect") return <Navigate to={outcome.to} />;
 *   return <>{children}</>;
 * }
 * const AuthRoute = (p) => <Guard allow="authenticated" redirectTo="/login" {...p} />;
 * const GuestRoute = (p) => <Guard allow="anonymous" redirectTo="/home" {...p} />;
 * const AdminRoute = (p) => <Guard allow={(u) => u.role === "admin"} redirectTo="/home" {...p} />;
 * ```
 *
 * - **`loading` ⇒ `wait`.** The answer is not in yet; defer (splash), never redirect — the bounce-to-login bug.
 * - **allowed ⇒ `render`.**
 * - **rejected ⇒ `redirect` to `redirectTo`.** A signed-in user on a guest route, an anonymous one on a private
 *   route, or a signed-in user lacking the capability, lands on the right screen instead of one they should never
 *   reach.
 *
 * @typeParam U - the authenticated user the session carries.
 * @typeParam Href - the app's route type.
 * @param session - the current {@link SessionState}.
 * @param options - which visitor is allowed and where a rejected one goes.
 * @returns the {@link GuardOutcome} the app renders.
 */
export function guardSession<U, Href = string>(
  session: SessionState<U>,
  options: GuardOptions<U, Href>,
): GuardOutcome<Href> {
  if (session.status === "loading") return { action: "wait" };
  const allow = options.allow;
  let allowed: boolean;
  if (typeof allow === "function")
    // A capability gate: only an authenticated user can satisfy it; an anonymous visitor has no user to inspect.
    allowed = session.status === "authenticated" && allow(session.user);
  else if (allow === "authenticated") allowed = session.status === "authenticated";
  else allowed = session.status === "anonymous";
  return allowed ? { action: "render" } : { action: "redirect", to: options.redirectTo };
}
