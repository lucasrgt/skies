import { expect, test as base } from "@playwright/test";

const QUALITY_EVENT_TYPES = new Set(["error", "warning"]);

/**
 * Observe browser failures and warning-tier console output until the returned assertion is called.
 * The canonical test fixture below calls it automatically for every case.
 */
export function watchPageQuality(page) {
  if (!page || typeof page.on !== "function" || typeof page.off !== "function") {
    throw new Error("watchPageQuality() requires a Playwright page.");
  }

  const issues = [];
  const failedDataResponses = new Set();
  const onResponse = (response) => {
    const status = response.status();
    const resourceType = response.request().resourceType();
    if (status >= 400 && (resourceType === "fetch" || resourceType === "xhr")) {
      failedDataResponses.add(response.url());
    }
  };
  const onPageError = (error) => {
    issues.push(`pageerror: ${error instanceof Error ? error.stack ?? error.message : String(error)}`);
  };
  const onConsole = (message) => {
    const type = message.type();
    if (!QUALITY_EVENT_TYPES.has(type)) return;

    const location = message.location();
    if (type === "error"
        && /^Failed to load resource: the server responded with a status of \d+/.test(message.text())
        && location?.url
        && failedDataResponses.has(location.url)) {
      return;
    }
    const source = location?.url
      ? ` (${location.url}${location.lineNumber ? `:${location.lineNumber}` : ""})`
      : "";
    issues.push(`console.${type}: ${message.text()}${source}`);
  };

  page.on("response", onResponse);
  page.on("pageerror", onPageError);
  page.on("console", onConsole);

  return function assertPageQuality() {
    page.off("response", onResponse);
    page.off("pageerror", onPageError);
    page.off("console", onConsole);
    if (issues.length === 0) return;

    throw new Error(
      `Browser emitted ${issues.length} unexpected error(s) or warning(s):\n${issues.map((issue) => `- ${issue}`).join("\n")}`,
    );
  };
}

/** Playwright's test fixture with fail-closed browser error and warning collection enabled for every case. */
export const test = base.extend({
  __skiesPageQuality: [async ({ page }, use) => {
    const assertPageQuality = watchPageQuality(page);
    try {
      await use();
    } finally {
      assertPageQuality();
    }
  }, { auto: true }],
});

export { expect };
