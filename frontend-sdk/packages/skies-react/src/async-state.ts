// AsyncState<T> — the frontend spine's core, the read-side analogue of the backend's Result<T>. A screen's
// ViewModel (its data door) exposes a resource as this discriminated union instead of ad-hoc isPending/isError
// booleans, so the View handles all four states by construction (an exhaustive switch / the <Resource> gate) —
// the same way an exhaustive `Result` match forces the sad path. Pure TS, design-system-agnostic.

export type AsyncState<T> =
  | { status: "loading" }
  | { status: "error"; message: string; retry?: () => void }
  | { status: "empty" }
  | { status: "ready"; data: T };

// The result shape this adapts — kept structural (not the react-query type) so the spine carries no data-lib
// dependency. It is wire over react-query (or any equivalent), not a dependency on it.
export interface QueryLike<T> {
  isPending: boolean;
  isError: boolean;
  data: T | undefined;
  refetch?: () => unknown;
}

/**
 * Project a query result into an AsyncState. `errorMessage` is the localized string the ViewModel supplies (the
 * spine never hardcodes copy). `isEmpty` opts the resource into a distinct "empty" state (e.g. an empty list);
 * omit it and a present-but-empty value is simply `ready` for the View to render.
 */
export function toAsyncState<T>(
  query: QueryLike<T>,
  opts: { errorMessage: string; isEmpty?: (data: T) => boolean },
): AsyncState<T> {
  if (query.isPending) return { status: "loading" };
  if (query.isError || query.data === undefined) {
    return {
      status: "error",
      message: opts.errorMessage,
      retry: query.refetch ? () => void query.refetch?.() : undefined,
    };
  }
  if (opts.isEmpty?.(query.data)) return { status: "empty" };
  return { status: "ready", data: query.data };
}

/** Project the ready payload while passing loading/error/empty through unchanged. */
export function mapAsyncState<T, U>(state: AsyncState<T>, project: (data: T) => U): AsyncState<U> {
  return state.status === "ready" ? { status: "ready", data: project(state.data) } : state;
}

/**
 * Combine several AsyncStates into one, for the screen that renders only when all its queries are in — the
 * composition every multi-query screen otherwise hand-rolls (and gets the precedence wrong). Precedence:
 * `error` > `loading` > `empty` > `ready` — an error is terminal (waiting on the other queries can't un-fail
 * it, so it surfaces immediately), loading defers, any empty empties the whole, and `ready` carries the data
 * tuple in argument order. The combined retry retries every errored source at once.
 */
export function combineAsyncStates<T extends readonly AsyncState<unknown>[]>(
  ...states: T
): AsyncState<{ [K in keyof T]: T[K] extends AsyncState<infer U> ? U : never }> {
  const errors = states.filter(
    (s): s is { status: "error"; message: string; retry?: () => void } => s.status === "error",
  );
  const first = errors[0];
  if (first !== undefined) {
    const retries = errors.map((s) => s.retry).filter((r): r is () => void => r !== undefined);
    return {
      status: "error",
      message: first.message,
      retry: retries.length > 0 ? () => retries.forEach((r) => r()) : undefined,
    };
  }
  if (states.some((s) => s.status === "loading")) return { status: "loading" };
  if (states.some((s) => s.status === "empty")) return { status: "empty" };
  return {
    status: "ready",
    data: states.map((s) => (s as { status: "ready"; data: unknown }).data) as never,
  };
}
