import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import tsParser from "@typescript-eslint/parser";
import tsPlugin from "@typescript-eslint/eslint-plugin";
import tanstackQuery from "@tanstack/eslint-plugin-query";
import noSecrets from "eslint-plugin-no-secrets";
import sonarjs from "eslint-plugin-sonarjs";
import vitest from "@vitest/eslint-plugin";

// Lint config for the canonical example (examples/sample-app/frontend/web), executed from the repository root by
// `npm run lint`. The root location keeps the example inside ESLint's base path while dependencies remain owned
// by frontend-sdk; the rule self-tests run as a separate leg.
//
// Curated community kit alongside the SKYFE rules. Two of those compose cleanly with the SKYFE rules:
//   - @tanstack/eslint-plugin-query — react-query correctness (exhaustive deps, stable keys, no rest-destructure);
//     the SKYFE rules cover architecture, this covers RQ usage — complementary, not overlapping.
//   - eslint-plugin-no-secrets — entropy-based hardcoded-secret detection (the .env discipline, enforced in code).
// The SKYFE plugin is CommonJS; load it via createRequire. Its recommended config carries the accessibility floor
// (jsx-a11y's recommended set at error), so the sample gets a11y the way an app does: by extending it.
const require = createRequire(import.meta.url);
const skies = require("./packages/eslint-plugin/index.cjs");
const SAMPLE = ["examples/sample-app/frontend/web/**/*.{ts,tsx}", "examples/sample-app/.specs/*/e2e/**/*.{ts,tsx}"];

export default [
  { ignores: ["**/node_modules/**", "packages/eslint-plugin/**"] },
  // react-query correctness (parser comes from the sample block below, which these merge onto).
  ...tanstackQuery.configs["flat/recommended"],
  // Skies' recommended, exactly as an adopter gets it: the SKYFE rules plus the jsx-a11y floor.
  { ...skies.configs.recommended, files: SAMPLE },
  {
    files: SAMPLE,
    languageOptions: {
      parser: tsParser,
      ecmaVersion: 2022,
      sourceType: "module",
      // type-aware lint (projectService) — required by @typescript-eslint/no-floating-promises.
      parserOptions: { ecmaFeatures: { jsx: true }, projectService: true, tsconfigRootDir: fileURLToPath(new URL("..", import.meta.url)) },
    },
    plugins: { "no-secrets": noSecrets, sonarjs, "@typescript-eslint": tsPlugin },
    rules: {
      // promise safety (type-aware) — an unhandled promise is a silent failure; `void p` opts out explicitly.
      "@typescript-eslint/no-floating-promises": "error",
      // curated community kit
      "no-secrets/no-secrets": ["error", { tolerance: 4.5 }],
      "sonarjs/no-identical-functions": "warn",
      "sonarjs/no-duplicated-branches": "warn",
      "sonarjs/cognitive-complexity": ["warn", 25],
    },
  },
  // test hygiene — no .only/.skip leaking into the suite (the @vitest recommended set). The sample's tests all live in
  // its specs.
  {
    files: ["examples/sample-app/.specs/*/e2e/**/*.test.{ts,tsx}"],
    plugins: { vitest },
    rules: { ...vitest.configs.recommended.rules },
  },
];
