// The session's WRITE side — the one seam SKYFE016 polices toward, graduated from the hostpoint pilot's
// lib/session. The read side (SessionState/toSessionState) projects "who is signed in"; this owns "the token
// changed": persist it, and reset the session cache IN THE SAME MOVE, so the scattered-write bug (a sign-in
// that forgets to reset the `me` query and bounces the fresh user back to login) is unrepresentable, not
// merely linted. Structural like the rest of the spine: the client call and the token sink are injected ports —
// the spine depends on no transport and no router. The refresh token never reaches JS: it is an httpOnly cookie the
// browser sends with the refresh request, so the seam persists nothing.

import { singleFlight } from "./single-flight";

/** The capabilities a session seam composes over, injected so the spine stays dependency-free. */
export interface SessionSeamPorts {
  /** Sink for the bearer token (e.g. the generated client's `setAccessToken`); `null` clears it. */
  setAccessToken: (token: string | null) => void;
  /** Re-mint the access token (the generated `refresh` endpoint). The refresh cookie rides the request, so the
   * call takes no argument. */
  refresh: () => Promise<AuthTokens | null | undefined>;
  /** ROTATION reset — LIGHT. The SAME identity got a fresh token (a boot bootstrap, a 401-refresh), so only the
   * session-shaped caches need re-reading (e.g. `queryClient.resetQueries` over `me`); the rest of the cache is
   * still that user's and stays warm — no blank flash. Runs on {@link SessionSeam.bootstrapSession}. */
  onSessionChanged?: () => void;
  /** IDENTITY reset — TOTAL. The identity ITSELF changed: an explicit sign-in (a DIFFERENT user may have
   * authenticated on this client) or a sign-out. Wipe the whole cache (e.g. `queryClient.clear()`) so the prior
   * user's data can never bleed into the next session — the hostpoint bug that a sign-out→sign-in on one client
   * leaked user A's cache to user B, "fixed" there by splitting the app in two. Runs on {@link SessionSeam.signIn}
   * and {@link SessionSeam.clearSession}. Required because substituting the light rotation reset recreates the
   * cross-identity cache leak this split exists to prevent. */
  onIdentityChanged: () => void;
}

/** What a login/refresh response carries for the seam: the bearer. The refresh token stays in its httpOnly cookie. */
export interface AuthTokens {
  accessToken?: string;
}

/** The seam's surface — the only ways the app may move the session. The two authenticating doors are split on
 * purpose: an explicit {@link signIn} is an IDENTITY change (total wipe), while
 * {@link bootstrapSession} is a ROTATION (light reset). */
export interface SessionSeam {
  /** Persist a session from an explicit SIGN-IN / sign-up response: bearer to the sink, then the IDENTITY reset
   * (total wipe) — a different user may have authenticated on this client, so the
   * prior user's cache is dropped entirely before the fresh `me` refetches. This is the identity door; the app
   * literally cannot authenticate a user without the wipe. */
  signIn: (result: unknown) => Promise<void>;
  /** Re-mint the access token from the refresh cookie — a ROTATION of the SAME identity, so only the light reset
   * runs and the screen stays warm. Returns whether a session was restored. Safe on every app start — the API
   * rotates the refresh cookie on each call, so the next bootstrap uses the latest one. */
  bootstrapSession: () => Promise<boolean>;
  /** Drop the session locally (the server clears the cookie / revokes on its side) — an IDENTITY change (total
   * wipe): the next user starts on a clean cache. */
  clearSession: () => Promise<void>;
}

/**
 * Build the app's session seam (its `lib/session`) from the injected ports:
 *
 * ```ts
 * export const session = createSessionSeam({
 *   setAccessToken,
 *   refresh: () => refresh(),
 *   // rotation: light — the same user, a fresh token; keep the screen warm.
 *   onSessionChanged: () => queryClient.resetQueries({ queryKey: getMeQueryKey() }),
 *   // identity: total — a different user may have signed in / out; drop everything.
 *   onIdentityChanged: () => queryClient.clear(),
 * });
 * ```
 */
export function createSessionSeam(ports: SessionSeamPorts): SessionSeam {
  const persistTokens = (result: unknown): void => {
    const tokens = (result ?? undefined) as AuthTokens | undefined;
    if (tokens?.accessToken) ports.setAccessToken(tokens.accessToken);
  };

  const signIn = async (result: unknown): Promise<void> => {
    persistTokens(result);
    ports.onIdentityChanged(); // a (possibly) new identity — wipe the prior user's cache entirely
  };

  // Single-flighted: a cold start that double-invokes the boot effect (React StrictMode in dev) or a bootstrap
  // racing the client's 401-interceptor would otherwise fire TWO refresh rotations — and the backend's
  // theft-detection burns the whole session family when it sees the spent token replayed (the SKYFE029 hazard at
  // boot). Concurrent callers share the one rotation; the gate reopens once it settles, so a later, genuine
  // re-bootstrap still runs.
  const bootstrapSession = singleFlight(async (): Promise<boolean> => {
    try {
      persistTokens(await ports.refresh());
      ports.onSessionChanged?.(); // rotation of the SAME identity — light reset, not the identity wipe
      return true;
    } catch {
      return false;
    }
  });

  return {
    signIn,
    bootstrapSession,
    async clearSession() {
      ports.setAccessToken(null);
      ports.onIdentityChanged(); // the identity is gone — wipe so the next user starts clean
    },
  };
}
