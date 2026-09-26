import { singleFlight } from "./single-flight";

/** The capabilities a session seam composes over, injected so the spine stays dependency-free. */
export interface SessionSeamPorts {
  /** Sink for the bearer token (e.g. the generated client's `setAccessToken`); `null` clears it. */
  setAccessToken: (token: string | null) => void;
  /** Re-mint the access token (the generated `refresh` endpoint). The refresh cookie rides the request, so the
   * call takes no argument. */
  refresh: () => Promise<AuthTokens | null | undefined>;
  /** Refresh session-shaped caches after rotation; other data still belongs to the same identity. */
  onSessionChanged?: () => void | Promise<void>;
  /** Clear all user-owned caches on sign-in and sign-out, before exposing the next identity's data. */
  onIdentityChanged: () => void | Promise<void>;
}

/** What a login/refresh response carries for the seam: the bearer. The refresh token stays in its httpOnly cookie. */
export interface AuthTokens {
  accessToken?: string;
}

/** Explicit identity changes clear all caches; refresh only invalidates session-shaped data. */
export interface SessionSeam {
  /** Stores the sign-in response and clears prior-identity caches. */
  signIn: (result: unknown) => Promise<void>;
  /** Shares one refresh per identity and ignores replies from an earlier identity. */
  bootstrapSession: () => Promise<boolean>;
  /** Clears local credentials and caches. The caller also revokes the server session. */
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
  let identity = 0;

  const persistTokens = (result: unknown): boolean => {
    const tokens = result as AuthTokens | null | undefined;
    if (!tokens?.accessToken) return false;
    ports.setAccessToken(tokens.accessToken);
    return true;
  };

  const refreshForIdentity = () => {
    const revision = identity;
    return singleFlight(async (): Promise<boolean> => {
      try {
        const tokens = await ports.refresh();
        if (revision !== identity || !persistTokens(tokens)) return false;
        await ports.onSessionChanged?.();
        return revision === identity;
      } catch {
        return false;
      }
    });
  };
  let bootstrap = refreshForIdentity();

  const changeIdentity = () => {
    identity++;
    bootstrap = refreshForIdentity();
  };

  return {
    async signIn(result: unknown) {
      changeIdentity();
      if (!persistTokens(result)) ports.setAccessToken(null);
      await ports.onIdentityChanged();
    },
    bootstrapSession: () => bootstrap(),
    async clearSession() {
      changeIdentity();
      ports.setAccessToken(null);
      await ports.onIdentityChanged();
    },
  };
}
