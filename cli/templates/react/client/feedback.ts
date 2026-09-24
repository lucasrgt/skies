// The one door for transient user feedback (toasts/banners) — the "one seam" shape applied to
// notifications. The app picks its toast library and wires it ONCE at boot (wireFeedback); everything below
// the shell — the mutation defaults in lib/query.ts, any ViewModel — speaks to this seam and never imports a
// toast lib directly. Swapping the library is a one-file change no ViewModel notices.

/** What a feedback sink renders. The shell provides one at boot (e.g. sonner's toast.success / toast.error). */
export interface FeedbackSink {
  success(message: string): void;
  error(message: string): void;
}

// Until the shell wires a real sink, errors fall back to the console — feedback degrades visibly, never
// silently (a dev booting without wireFeedback still sees every failure).
let sink: FeedbackSink = {
  success: () => undefined,
  error: (message) => console.error(`[feedback] ${message}`),
};

/** Install the app's toast sink — called once at boot by the shell. */
export function wireFeedback(next: FeedbackSink): void {
  sink = next;
}

/** The seam the mutation defaults (and any ViewModel) call — success and error notes through one door. */
export const feedback: FeedbackSink = {
  success: (message) => sink.success(message),
  error: (message) => sink.error(message),
};
