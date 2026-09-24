import type { Page } from "@playwright/test";

export { expect } from "@playwright/test";

/** Playwright's test fixture with fail-closed browser error and warning collection enabled for every case. */
export const test: typeof import("@playwright/test").test;

/**
 * Observe browser failures and warning-tier console output until the returned assertion is called.
 * Prefer the canonical exported test fixture, which closes this assertion automatically.
 */
export function watchPageQuality(page: Pick<Page, "on" | "off">): () => void;
