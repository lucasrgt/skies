// RequiredParam — the param-presence union the routing harness (SKYFE018) steers toward. A route hit without its
// id (a bookmark, a stale link) otherwise renders a "ghost" screen bound to an empty value; projecting the raw
// param through this closed union makes the missing case a branch the route must write, not a state it forgets.
// Router-agnostic like the rest of the spine: it takes the raw value, not a router.

export type RequiredParam =
  | { status: "missing" }
  | { status: "ready"; value: string };

/**
 * Project a raw route/search param into a {@link RequiredParam}. Routers hand params over loosely —
 * `string | undefined` from React Router's `useParams()`, an array when a repeated search key is parsed — and this
 * normalizes all of it: absent, empty, or an empty array is `missing`; an array yields its first entry.
 *
 * The route branches declaratively on the result, the same shape SKYFE018 enforces:
 *
 * ```tsx
 * const id = requiredParam(params.chatId);
 * if (id.status === "missing") return <Navigate to="/messaging" />;
 * return <Chat chatId={id.value} />;
 * ```
 */
export function requiredParam(raw: string | string[] | undefined): RequiredParam {
  const value = Array.isArray(raw) ? raw[0] : raw;
  if (value === undefined || value === "") return { status: "missing" };
  return { status: "ready", value };
}
