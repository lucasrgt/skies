// Same-origin in a real app; the fallback gives jsdom/MSW a stable absolute origin.
export const SAMPLE_API_BASE = globalThis.location?.origin ?? "http://sample.test";
