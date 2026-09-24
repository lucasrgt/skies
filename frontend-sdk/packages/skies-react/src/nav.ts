// Navigation primitives the spine graduates from app pilots. Router-AGNOSTIC by the same trick the rest of the
// spine uses (cf. QueryLike): they take the router's capabilities structurally, so they bind to TanStack
// Router, React Router, or any equivalent without the spine depending on one.

/**
 * The router capabilities {@link safeBack} needs, taken structurally so the spine depends on no specific router.
 *
 * @typeParam Href - the app's route type (a string on every router); generic so a typed-routes setup keeps its
 * compile-time checking through `safeBack`.
 */
export interface BackRouter<Href = string> {
  /** Whether there is in-app history to pop. */
  canGoBack: () => boolean;
  /** Pop one entry off the back stack. */
  back: () => void;
  /** Navigate to `to`, replacing the current entry (no new history). */
  replace: (to: Href) => void;
}

/**
 * A "go back" that always lands somewhere. After in-app navigation the history pops as usual; but a fresh load /
 * deep link / refresh of a deep route leaves NO in-app history, so a bare `back()` is a no-op (or leaves the app)
 * and the "Back" button appears dead. When there is nothing to pop, `replace` to `fallback` instead.
 *
 * Bind this to your router in a one-line app hook (e.g. `useGoBack`) and route every "Back" affordance through it —
 * a bare `history.back()` is the dead-button bug on a deep link. With TanStack Router:
 * `safeBack({ canGoBack: () => router.history.canGoBack(), back: () => router.history.back(),
 * replace: (to) => router.navigate({ to, replace: true }) }, "/")`.
 *
 * @typeParam Href - the app's route type.
 * @param router - the structural router capabilities.
 * @param fallback - where to land when there is no history to pop (e.g. the app root, which re-resolves the
 * session and redirects to the correct home).
 */
export function safeBack<Href = string>(router: BackRouter<Href>, fallback: Href): void {
  if (router.canGoBack()) router.back();
  else router.replace(fallback);
}
