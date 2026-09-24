import axios, {
  type AxiosRequestConfig,
  type AxiosError,
  type InternalAxiosRequestConfig,
} from "axios";

// The single HTTP seam every generated slice-hook calls through — the orval mutator. The opinion lives here
// (not in a fork of any tool): the base URL, the bearer token, and a uniform error shape. The generated
// *.gen.ts hooks are boring plumbing on top of this; all behaviour lives above, in the ViewModels.
//
// withCredentials so the session cookie (set by the API on login) rides cross-origin requests. X-Client: web
// is the Skies.Framework.Auth convention: the API delivers the refresh token as an httpOnly cookie ONLY when the
// request carries it; without it the refresh rides the response body, where page script could read it, and the
// cookie session never forms.
const instance = axios.create({
  withCredentials: true,
  headers: { "X-Client": "web" },
});

// The base URL is INJECTED by the app shell at boot (configureClient), never read from build config here — so
// this data-door module stays free of env wiring and imports cleanly in jsdom. The localhost default keeps tests
// + pre-configure dev working; SKYFE020 blesses exactly this injectable-default shape.
instance.defaults.baseURL = "http://localhost:8080";

/** Point the client at the resolved API base URL. Called once at app start by the shell — the same
 * push-don't-pull seam as setAccessToken; the data layer never imports build config. */
export function configureClient(apiUrl: string): void {
  if (apiUrl) instance.defaults.baseURL = apiUrl;
}

let accessToken: string | null = null;

/** Set (or clear) the bearer token the client sends — called only by the session seam (lib/session). */
export function setAccessToken(token: string | null): void {
  accessToken = token;
}

// ── Token refresh — the SEAM rotates, the client only retries ───────────────
// The 401 interceptor restores the session transparently, but it does NOT know HOW to rotate. The rotation (an
// empty post; the refresh rides the httpOnly cookie) is the session seam's concern — and it rotates SINGLE-FLIGHT
// there (its bootstrapSession), so concurrent 401s share ONE in-flight rotation instead of replaying a spent token
// and tripping the backend's theft detection (which burns the whole session family). The shell registers that
// rotation here at boot, so the rotation logic lives in exactly one place — the seam, never forked into this
// transport file. Until the shell wires it, a 401 settles to 401.
type TokenRefresher = () => Promise<boolean>;
let refreshSession: TokenRefresher | null = null;

/** Register the session seam's rotation (its single-flighted `bootstrapSession`) as the 401 refresher —
 * called once at boot by the shell: `setTokenRefresher(session.bootstrapSession)`. The refresher rotates and
 * pushes the fresh bearer through `setAccessToken`; it resolves true when a live session was restored. */
export function setTokenRefresher(refresher: TokenRefresher): void {
  refreshSession = refresher;
}

// On a 401, transparently rotate once (through the injected seam) and replay the request — restores the
// session on a cold load (the in-memory bearer is gone, the refresh credential survives) and rides over a
// mid-session expiry without bouncing to login. The auth routes are exempt and each request retries at most
// once, so a genuinely anonymous caller settles to 401 instead of looping.
instance.interceptors.response.use(
  (response) => response,
  async (error: AxiosError) => {
    const original = error.config as (InternalAxiosRequestConfig & { _retried?: boolean }) | undefined;
    const url = original?.url ?? "";
    const isAuthRoute = url.includes("/refresh") || url.includes("/login");
    if (error.response?.status === 401 && original && !original._retried && !isAuthRoute && refreshSession) {
      original._retried = true;
      // The seam rotates (single-flight) and pushes the new bearer through setAccessToken; replay with it.
      const restored = await refreshSession();
      if (restored && accessToken) {
        original.headers.set("Authorization", `Bearer ${accessToken}`);
        return instance.request(original);
      }
    }
    return Promise.reject(error);
  },
);

/** The mutator orval wires every endpoint through: inject auth, return the body. */
export const skiesClient = async <T>(config: AxiosRequestConfig): Promise<T> => {
  const response = await instance.request<T>({
    ...config,
    headers: {
      ...config.headers,
      ...(accessToken ? { Authorization: `Bearer ${accessToken}` } : {}),
    },
  });
  return response.data;
};

export default skiesClient;
