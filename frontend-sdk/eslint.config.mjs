import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import tsParser from "@typescript-eslint/parser";
import tsPlugin from "@typescript-eslint/eslint-plugin";
import tanstackQuery from "@tanstack/eslint-plugin-query";
import noSecrets from "eslint-plugin-no-secrets";
import sonarjs from "eslint-plugin-sonarjs";
import vitest from "@vitest/eslint-plugin";

// Lint config for the canonical example (examples/sample-app/frontend), executed from the repository root by
// `npm run lint`. The root location keeps the example inside ESLint's base path while dependencies remain owned
// by frontend-sdk; the rule self-tests run as a separate leg.
//
// Curated community kit alongside the SKYFE rules. Two of those compose cleanly with the SKYFE rules:
//   - @tanstack/eslint-plugin-query — react-query correctness (exhaustive deps, stable keys, no rest-destructure);
//     the SKYFE rules cover architecture, this covers RQ usage — complementary, not overlapping.
//   - eslint-plugin-no-secrets — entropy-based hardcoded-secret detection (the .env discipline, enforced in code).
// The SKYFE plugin is CommonJS; load it via createRequire.
const require = createRequire(import.meta.url);
const skies = require("./packages/eslint-plugin/index.cjs");
// Accessibility — web (DOM) uses jsx-a11y (alt / aria / href). The mobile RN counterpart (react-native-a11y)
// is dropped until it publishes an eslint-9 peer: its latest release still caps eslint at 8, and we keep the
// install ERESOLVE-clean rather than pin a peer-dependency escape hatch. Restore the mobile block (and a
// toWarn helper) once a maintained eslint-9 RN-a11y plugin exists.
const jsxA11y = require("eslint-plugin-jsx-a11y");

export default [
  { ignores: ["**/node_modules/**", "packages/eslint-plugin/**"] },
  // react-query correctness (parser comes from the sample block below, which these merge onto).
  ...tanstackQuery.configs["flat/recommended"],
  {
    files: ["examples/sample-app/frontend/{core,web}/**/*.{ts,tsx}", "examples/sample-app/.specs/*/e2e/**/*.{ts,tsx}"],
    languageOptions: {
      parser: tsParser,
      ecmaVersion: 2022,
      sourceType: "module",
      // type-aware lint (projectService) — required by @typescript-eslint/no-floating-promises.
      parserOptions: { ecmaFeatures: { jsx: true }, projectService: true, tsconfigRootDir: fileURLToPath(new URL("..", import.meta.url)) },
    },
    plugins: { skies, "no-secrets": noSecrets, sonarjs, "@typescript-eslint": tsPlugin },
    rules: {
      // promise safety (type-aware) — an unhandled promise is a silent failure; `void p` opts out explicitly.
      "@typescript-eslint/no-floating-promises": "error",
      "skies/view-purity": "error",
      "skies/data-door": "error",
      "skies/viewmodel-platform-agnostic": "error",
      "skies/no-mock": "error",
      "skies/state-completeness": "error",
      "skies/i18n-completeness": "error",
      "skies/mutation-error-handled": "error",
      "skies/no-hardcoded-copy": "error",
      // The routing harness (SKYFE015–019 + 030) — declarative redirects, one session seam, a tri-state guard,
      // guarded params + Back, and no cast on a navigation target (the typed-routes mute button). Error-tier
      // (correctness), router-agnostic (expo + TanStack). The default a generated app gets.
      "skies/no-router-replace-in-effect": "error",
      "skies/session-one-door": "error",
      "skies/guard-tristate": "error",
      "skies/route-param-guard": "error",
      "skies/safe-back": "error",
      "skies/no-cast-navigation": "error",
      // The form-validation pair (SKYFE031–032) — every validation failure has a surface: the submit carries its
      // invalid path (the spine's submitOrReveal), and a <Controller> surfaces its field's error. Warn-tier on
      // entry (a single-screen form with all errors visible inline is legitimate); promote together once the
      // primitive absorbs the common case.
      "skies/submit-handles-invalid": "warn",
      "skies/controller-field-state": "warn",
      "skies/no-hardcoded-base-url": "error",
      // The security pair (SKYFE021–022) — the XSS door stays behind one sanitizing seam; a URL-supplied value
      // never becomes a navigation target without an allowlist. Error-tier (correctness, same bar as routing).
      "skies/no-raw-html": "error",
      "skies/no-open-redirect": "error",
      // The mutation band (SKYFE027–028) — the QueryClient carries the write-side defaults (invalidate on success,
      // feedback on error), and the hand-rolled `onSuccess: refetch` ritual those defaults obsolete is revealed.
      "skies/query-client-defaults": "error",
      "skies/no-manual-refetch-ritual": "warn",
      // The session-rotation door (SKYFE029) — refresh is consumed by ONE seam (the client's single-flight
      // interceptor / the session seam); a second rotation path trips the backend's theft detection.
      "skies/refresh-one-door": "error",
      // Every test lives in a spec (SKYFE036): a test call outside .specs/<id>-<slug>/e2e/ is flagged.
      "skies/tests-live-in-specs": "error",
      // curated community kit
      "no-secrets/no-secrets": ["error", { tolerance: 4.5 }],
      "sonarjs/no-identical-functions": "warn",
      "sonarjs/no-duplicated-branches": "warn",
      "sonarjs/cognitive-complexity": ["warn", 25],
    },
  },
  // a11y — web (DOM): jsx-a11y at error tier. aria-role checks DOM elements only (ignoreNonDOM): the ui kit's
  // `Text role=` is a typography role, not an ARIA role — components map their props internally.
  {
    files: ["examples/sample-app/frontend/web/**/*.{ts,tsx}"],
    plugins: { "jsx-a11y": jsxA11y },
    rules: { ...jsxA11y.flatConfigs.recommended.rules, "jsx-a11y/aria-role": ["error", { ignoreNonDOM: true }] },
  },
  // test hygiene — no .only/.skip leaking into the suite (the @vitest recommended set). The sample's tests all live in
  // its specs.
  {
    files: ["examples/sample-app/.specs/*/e2e/**/*.test.{ts,tsx}"],
    plugins: { vitest },
    rules: { ...vitest.configs.recommended.rules },
  },
];
