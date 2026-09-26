// Collapse concurrent calls of an async operation into ONE in-flight execution — the spine primitive behind
// single-flight refresh. The session-rotation credential (the httpOnly refresh cookie) is
// BURNED by parallel rotation: the backend's theft detection sees a spent token replayed and revokes the whole
// session family (the SKYFE029 hazard). Two things race it in practice: two 401s landing together (each wanting a
// refresh) and a cold-start firing the boot bootstrap twice (React StrictMode double-invokes effects in dev). A
// single-flight gate makes the second caller AWAIT the first's result instead of starting a second rotation.
// Pure and router-agnostic like the rest of the spine.

/**
 * Wrap `fn` so that while one call is in flight, every further call returns that SAME promise instead of invoking
 * `fn` again; once it settles (resolve OR reject), the gate reopens and the next call runs fresh.
 *
 * Use it for the refresh seam — the app's 401-interceptor `refreshAccessToken` and the session seam's boot
 * `bootstrapSession` are both single-flighted by this, so concurrent 401s / a double-invoked boot effect trigger
 * exactly one rotation:
 *
 * ```ts
 * const refreshAccessToken = singleFlight(async () => {
 *   const tokens = await refresh();        // the one rotation all concurrent callers share
 *   setAccessToken(tokens.accessToken);
 *   return tokens;
 * });
 * ```
 *
 * NOTE — coalesced callers share the FIRST call's arguments and result. That is exactly right for a rotation
 * (the credential rides the cookie / store, not the argument), but do not wrap an operation whose result must
 * differ per argument.
 *
 * @typeParam A - the wrapped function's argument tuple.
 * @typeParam R - the wrapped function's resolved value.
 * @param fn - the async operation to single-flight.
 * @returns a function with `fn`'s signature that shares one in-flight execution across concurrent callers.
 */
export function singleFlight<A extends unknown[], R>(
  fn: (...args: A) => Promise<R>,
): (...args: A) => Promise<R> {
  let inFlight: Promise<R> | null = null;
  return (...args: A): Promise<R> => {
    if (inFlight) return inFlight;
    // Wrap in an async IIFE so a synchronous throw inside `fn` becomes a rejected promise (still gated + cleared),
    // never an exception that escapes before the gate is armed.
    const run = (async () => fn(...args))();
    inFlight = run;
    // Clear on settle (resolve OR reject). Attach via then(cb, cb) — NOT `.finally`, whose derived promise would
    // re-reject and surface as an unhandled rejection; here both branches return undefined, so this consumer is
    // fully settled while the original `run` carries the result/rejection to the caller.
    const clear = (): void => {
      // Only clear our own flight — a defensive guard against reopening a gate a later call already re-armed.
      if (inFlight === run) inFlight = null;
    };
    run.then(clear, clear);
    return run;
  };
}
