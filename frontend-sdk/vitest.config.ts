import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

// Runs the spine (@skiesjs/react) tests AND the canonical example's specs (examples/sample-app/.specs/*/e2e). The
// example keeps every test in a spec, so its cases live there, not beside the code: its agnostic core (the ViewModel +
// the View) renders against the WEB `@/ui` components in jsdom; the spine + the generated client + i18n resolve to
// source. Root is the repo so the example (a sibling of frontend/) is in scope; the include globs keep the run to the
// real test files. The sample's `[runners.web]` runs this same config with a spec folder as the filter.
const r = (p: string) => fileURLToPath(new URL(p, import.meta.url));

export default defineConfig({
  server: { fs: { allow: [r("..")] } }, // allow loading the example (a sibling of frontend/) under the repo root
  test: {
    maxWorkers: 2,
    root: r(".."),
    environment: "jsdom",
    setupFiles: [r("./vitest.setup.ts")],
    include: [
      "frontend-sdk/packages/**/*.test.{ts,tsx}",
      "examples/sample-app/.specs/*/e2e/**/*.test.{ts,tsx}",
    ],
  },
  resolve: {
    alias: {
      "@skiesjs/react": r("./packages/skies-react/src/index.ts"),
      "@/client.gen/sample": r("../examples/sample-app/frontend/core/src/client.gen/sample.ts"),
      "@/i18n": r("../examples/sample-app/frontend/core/src/i18n.ts"),
      "@/ui": r("../examples/sample-app/frontend/web/src/ui/index.ts"),
      // The example lives at examples/ (a sibling of frontend/), so its direct bare imports can't reach
      // frontend/node_modules by node resolution — alias them to the framework's installed copies (their transitive
      // deps then resolve from there naturally). Boundary-matched, so "react" doesn't catch "react-i18next" etc.
      react: r("./node_modules/react"),
      "react-i18next": r("./node_modules/react-i18next"),
      i18next: r("./node_modules/i18next"),
      "@testing-library/react": r("./node_modules/@testing-library/react"),
      "@tanstack/react-query": r("./node_modules/@tanstack/react-query"),
      "react-hook-form": r("./node_modules/react-hook-form"),
      zod: r("./node_modules/zod"),
      "@hookform/resolvers": r("./node_modules/@hookform/resolvers"),
    },
  },
});
