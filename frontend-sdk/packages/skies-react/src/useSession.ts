import { useEffect, useState } from "react";

// The boot gate over the session seam. Runs the one-time bootstrap (re-mint the access token from the persisted
// refresh) and reports when it has settled. Mount at the app root and gate the navigator on `ready`, so no route
// fires an authed request before the session is restored — this is what makes an F5 on any route resume the
// session instead of 401-ing. Graduated from the hostpoint pilot.

/**
 * Settle the session before the navigator renders:
 *
 * ```tsx
 * const { ready } = useSession(session.bootstrapSession);
 * if (!ready) return <Splash />;
 * ```
 *
 * @param bootstrap - the seam's `bootstrapSession` (or any settle-the-session promise).
 * @param options - `timeoutMs` arms a fallback: a bootstrap that never settles (a hung socket on cold-start)
 * would otherwise pin the app on the splash FOREVER, with no path to login. After `timeoutMs`, the gate opens
 * anyway and the guard decides (anonymous → login); a late bootstrap success still set its token before the
 * fallback fired. Omit it to keep the original wait-forever behaviour.
 */
export function useSession(
  bootstrap: () => Promise<unknown>,
  options?: { timeoutMs?: number },
): { ready: boolean } {
  const [ready, setReady] = useState(false);
  const timeoutMs = options?.timeoutMs;

  useEffect(() => {
    let cancelled = false;
    const settle = () => {
      if (!cancelled) setReady(true);
    };
    // A rejected bootstrap is a settled (anonymous) session, not an unhandled error — absorb it; the seam's
    // own bootstrapSession already maps failure to `false`, this covers any custom promise the app passes.
    void bootstrap().catch(() => undefined).finally(settle);
    // The escape hatch: never let a hung bootstrap hold the navigator forever.
    const timer = timeoutMs != null ? setTimeout(settle, timeoutMs) : null;
    return () => {
      cancelled = true;
      if (timer != null) clearTimeout(timer);
    };
    // The bootstrap is a one-time boot gate by contract — re-running it on a new function identity would
    // re-mint mid-session.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return { ready };
}
