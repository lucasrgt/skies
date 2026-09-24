// The error-code → copy bridge — the runtime half of the error-code discipline. The backend ships every
// failure as an ErrorBody { error, code, message, fields } where `code` is a stable, language-neutral registry
// key (SKY0018/19) enumerated into the OpenAPI contract; the front owns the copy in an "api-errors" i18n
// namespace typed against the generated code union, so a missing code is a type error. This consumes that pair: code off the
// failed request, copy out of the catalog, a generic fallback when the error carries no known code — so a
// ViewModel never hardcodes a per-screen error string. Structural like the rest of the spine: the i18n
// instance is injected, not imported. Graduated from the hostpoint pilot.

/** The i18n capabilities the bridge needs, taken structurally (react-i18next's instance satisfies it). */
export interface I18nLike {
  /** Whether the catalog declares `key`. */
  exists: (key: string) => boolean;
  /** The localized copy for `key`. */
  t: (key: string) => string;
}

/** Where the bridge looks copy up; both have catalog-convention defaults. */
export interface ApiErrorCopyOptions {
  /** The namespace the error-code catalog lives in (default `api-errors`, the catalog's conventional namespace). */
  namespace?: string;
  /** The generic key used when the error carries no known code (default `common:state.loadError`). */
  fallbackKey?: string;
}

/**
 * The `ErrorBody.code` a failed request carries (it rides the axios error's response body), or null when the
 * error has no code — a network failure, a non-Skies endpoint, a crash.
 */
export function apiErrorCode(error: unknown): string | null {
  const data = (error as { response?: { data?: unknown } } | null | undefined)?.response?.data;
  if (data && typeof data === "object" && "code" in data) {
    const code = (data as { code?: unknown }).code;
    if (typeof code === "string" && code.length > 0) return code;
  }
  return null;
}

/**
 * Localized copy for a failed request: the catalog entry for its code, the generic fallback when the error
 * carries no known code, and null when there is no error at all — so a ViewModel writes
 * `error: apiErrorCopy(mut.error, i18n)` and the error surface follows the catalog, never a hardcoded string.
 */
export function apiErrorCopy(error: unknown, i18n: I18nLike, options?: ApiErrorCopyOptions): string | null {
  if (!error) return null;
  const code = apiErrorCode(error);
  const key = code ? `${options?.namespace ?? "api-errors"}:${code}` : null;
  return key && i18n.exists(key) ? i18n.t(key) : i18n.t(options?.fallbackKey ?? "common:state.loadError");
}
