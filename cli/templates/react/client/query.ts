import { MutationCache, QueryClient } from "@tanstack/react-query";

import { feedback } from "./feedback";

// The write-side defaults every mutation inherits. A successful mutation marks EVERY query stale
// (TanStack refetches the active ones immediately), so no screen hand-rolls `onSuccess: refetch` and no list
// is ever one F5 behind its server — the safe, slightly-wasteful default that is always correct. A failed
// mutation always surfaces through the feedback seam, so no failure is silent.
// Targeted invalidation and optimistic updates remain the per-screen opt-in LAYERED ABOVE this default for the
// screen that proves it needs them, never a replacement for it.

declare module "@tanstack/react-query" {
  interface Register {
    mutationMeta: {
      /** Skip the success note for this mutation (a sign-in, a drag reorder — the UI change IS the feedback). */
      silent?: boolean;
      /** Skip the ERROR note — legal ONLY when the failure is a modeled, visible state the screen renders
       * (an anonymous probe settling onto the login screen), never a way to hide a real failure. */
      expectedFailure?: boolean;
    };
  }
}

/** The copy the defaults speak — resolved by the shell (i18n), so this seam carries no i18n dependency. */
export interface MutationCopy {
  /** The generic success note ("Saved"). */
  saved(): string;
  /** The failure note — receives the error so the app can map an ErrorBody code to localized copy. */
  failed(error: unknown): string;
}

/** Build the app's QueryClient with the convention's mutation defaults wired. */
export function createQueryClient(copy: MutationCopy): QueryClient {
  const queryClient: QueryClient = new QueryClient({
    mutationCache: new MutationCache({
      onSuccess: (_data, _variables, _context, mutation) => {
        void queryClient.invalidateQueries();
        if (mutation.meta?.silent !== true) feedback.success(copy.saved());
      },
      // A mutation failure ALWAYS surfaces — the only opt-out is `meta.expectedFailure`, for the mutation
      // whose failure is a modeled, visible state (not an error to announce). A screen that also reads
      // .isError just adds a richer inline surface on top — double feedback beats the silent kind.
      onError: (error, _variables, _context, mutation) => {
        if (mutation.meta?.expectedFailure !== true) feedback.error(copy.failed(error));
      },
    }),
  });
  return queryClient;
}
