"use strict";

// Regression cases from calibrating the SKYFE rules on real applications (docs/decisions/skies-5-rule-calibration.md):
// each false positive a real app produced is pinned valid here, next to the violation the rule must still catch, and
// the route-file layouts (TanStack Router's `src/routes/**`, React Router's `routes/`) are pinned for the route-scoped
// rules. Required by index.test.cjs.

const { RuleTester } = require("eslint");
const tsParser = require("@typescript-eslint/parser");
const plugin = require("./index.cjs");

const ruleTester = new RuleTester({
  languageOptions: {
    parser: tsParser,
    ecmaVersion: 2022,
    sourceType: "module",
    parserOptions: { ecmaFeatures: { jsx: true } },
  },
});

// SKYFE011 — only locale-named catalogs are compared. A real feature file keeps one locale-keyed object beside
// error-code maps and key tables; those lookups are not locales (55 false positives in one app before the fix).
ruleTester.run("i18n-completeness (calibration)", plugin.rules["i18n-completeness"], {
  valid: [
    {
      filename: "auth.i18n.ts",
      code: `const messages = { "pt-BR": { entrar: "Entrar", erro: "Erro" }, "en-US": { entrar: "Sign in", erro: "Error" } } as const;
const apiErrorByCode: Record<string, string> = { "auth.invalid_credentials": "erro" };
export const apiErrors = { exists: (k: string) => k.length > 0, t: (k: string) => k };
export const apiErrorFallbackKey = "api-errors:fallback";`,
    },
    {
      filename: "x.i18n.ts",
      code: `export const ptBR = { a: "1" }; export const enUS = { a: "x" }; const errorByCode = { "x.y": "a" };`,
    },
    { filename: "x.i18n.ts", code: `export const pt = { a: "1" }; export const en_US = { a: "x" };` },
  ],
  invalid: [
    {
      filename: "auth.i18n.ts",
      code: `const messages = { "pt-BR": { entrar: "Entrar", sair: "Sair" }, "en-US": { entrar: "Sign in" } } as const;`,
      errors: [{ messageId: "missing" }],
    },
    {
      filename: "x.i18n.ts",
      code: `export const ptBR = { a: "1", b: "2" } satisfies Catalog; export const enUS = { a: "x" } satisfies Catalog;`,
      errors: [{ messageId: "missing" }],
    },
  ],
});

// SKYFE036 — a Playwright fixture file extends the runner (`base.extend`, `test.step`), it declares no case; and a
// case that imports the app's re-exported, extended `test` is still a test (63 legacy spec files were missed).
ruleTester.run("tests-live-in-specs (calibration)", plugin.rules["tests-live-in-specs"], {
  valid: [
    {
      filename: "e2e/support/skies-playwright.mjs",
      code: `import { expect, test as base } from "@playwright/test"; export const test = base.extend({ page: async ({ page }, use) => { await use(page); } }); export { expect };`,
    },
    {
      filename: "e2e/support/smoke.ts",
      code: `import { test } from "@playwright/test"; export async function open(page: unknown) { await test.step("open", async () => {}); }`,
    },
    {
      filename: "e2e/support/hooks.ts",
      code: `import { test } from "@playwright/test"; test.beforeEach(async () => {}); test.describe.configure({ mode: "serial" });`,
    },
    // A helper that merely shares the name, called without a title and a body, is not a test.
    { filename: "src/tree.ts", code: `import { describe } from "./describe"; export const d = describe(node, { depth: 2 });` },
    {
      filename: ".specs/0001-auth/e2e/auth.test.ts",
      code: `import { test } from "../support/fixtures"; test("FM-1: registers", async ({ page }) => {});`,
    },
  ],
  invalid: [
    {
      filename: "e2e/auth.spec.ts",
      code: `import { expect, test } from "./support/skies-playwright.mjs"; test("registers", async ({ page }) => { expect(page).toBeTruthy(); });`,
      errors: [{ messageId: "outsideSpec" }],
    },
    {
      filename: "src/lib/accept.test.ts",
      code: `import { describe, it } from "node:test"; describe("accept", () => { it("negotiates", () => {}); });`,
      errors: [{ messageId: "outsideSpec" }],
    },
    {
      filename: "e2e/login.spec.ts",
      code: "import { test } from '~/fixtures'; test.describe(`login`, () => {});",
      errors: [{ messageId: "outsideSpec" }],
    },
  ],
});

// SKYFE020 — Playwright's `use.baseURL` is where the test runner points, not the app's API client.
ruleTester.run("no-hardcoded-base-url (calibration)", plugin.rules["no-hardcoded-base-url"], {
  valid: [
    { filename: "playwright.config.ts", code: `export default defineConfig({ use: { baseURL: "http://127.0.0.1:4173" } });` },
    { filename: "clients/web/playwright.design.config.ts", code: `export default { use: { baseURL: "http://localhost:28186" } };` },
  ],
  invalid: [
    {
      filename: "src/lib/api.ts",
      code: `export const api = axios.create({ baseURL: "http://localhost:8080" });`,
      errors: [{ messageId: "hardcoded" }],
    },
  ],
});

// SKYFE002 — a test that drives a generated hook to prove the client's transport exercises the data door.
ruleTester.run("data-door (calibration)", plugin.rules["data-door"], {
  valid: [
    { filename: "src/lib/skies-client.test.ts", code: `import { useMe } from "@/client.gen/marombas";` },
    { filename: ".specs/0003-session/e2e/refresh.tsx", code: `import { useMe } from "@/client.gen/marombas";` },
  ],
  invalid: [
    { filename: "src/lib/helpers.ts", code: `import { useMe } from "@/client.gen/marombas";`, errors: [{ messageId: "offdoor" }] },
  ],
});

// Route files: TanStack Router's file-based `src/routes/**` and React Router's `routes/` are the navigation layer too.
ruleTester.run("route-param-guard (route layouts)", plugin.rules["route-param-guard"], {
  valid: [
    // A strict TanStack read is guaranteed by the matched route.
    { filename: "src/routes/_app/planos/$id.tsx", code: `function R() { const { id } = Route.useParams(); return <Plan id={id} />; }` },
    // A throw on the absence test (TanStack's notFound, an error boundary) and an invariant keep the ghost off.
    { filename: "src/routes/items.tsx", code: `function R() { const { itemId } = useParams({ strict: false }); if (!itemId) throw notFound(); return <I id={itemId} />; }` },
    { filename: "src/routes/items.tsx", code: `function R() { const { itemId } = useParams(); invariant(itemId, "itemId"); return <I id={itemId} />; }` },
    // A test beside the routes and a folder merely named like one are not route files.
    { filename: "src/routes/routes.test.tsx", code: `function R() { const { itemId } = useParams(); return <I id={itemId} />; }` },
    { filename: "src/myroutes/items.tsx", code: `function R() { const { itemId } = useParams(); return <I id={itemId} />; }` },
  ],
  invalid: [
    {
      filename: "src/routes/_app/items/$itemId.tsx",
      code: `function R() { const { itemId } = useParams({ strict: false }); return <I id={itemId} />; }`,
      errors: [{ messageId: "unguarded" }],
    },
    {
      filename: "src/routes/chat.tsx",
      code: `function R() { const { chatId } = useParams() as { chatId?: string }; return <Chat id={chatId} />; }`,
      errors: [{ messageId: "unguarded" }],
    },
    // `return null` is the blank ghost itself, not a guard.
    {
      filename: "app/routes/chat.tsx",
      code: `function R() { const { chatId } = useParams(); if (!chatId) return null; return <Chat id={chatId} />; }`,
      errors: [{ messageId: "unguarded" }],
    },
  ],
});

ruleTester.run("safe-back (route layouts)", plugin.rules["safe-back"], {
  valid: [
    {
      filename: "src/routes/settings.tsx",
      code: `function R() { const router = useRouter(); const canBack = useCanGoBack(); const back = () => (canBack ? router.history.back() : router.navigate({ to: "/" })); return null; }`,
    },
  ],
  invalid: [
    { filename: "src/routes/settings.tsx", code: `const onBack = () => window.history.back();`, errors: [{ messageId: "bareBack" }] },
    {
      filename: "src/routes/settings.tsx",
      code: `function R() { const navigate = useNavigate(); const onBack = () => navigate(-1); return null; }`,
      errors: [{ messageId: "bareBack" }],
    },
  ],
});

ruleTester.run("no-router-replace-in-effect (route layouts)", plugin.rules["no-router-replace-in-effect"], {
  valid: [],
  invalid: [
    {
      filename: "src/routes/GoogleCallbackScreen.tsx",
      code: `function R() { const navigate = useNavigate(); useEffect(() => { if (done) void navigate({ to: "/" }); }, [done]); return null; }`,
      errors: [{ messageId: "effectReplace" }],
    },
  ],
});

// SKYFE022 — only the navigation TARGET counts; TanStack's `Route.useSearch()` is a URL read like `useSearch()`.
ruleTester.run("no-open-redirect (route layouts)", plugin.rules["no-open-redirect"], {
  valid: [
    // Forwarding `redirect` in the next page's search keeps the hop allowlisted; it is not the target.
    {
      filename: "src/routes/login.tsx",
      code: `function R() { const { redirect } = Route.useSearch(); const navigate = useNavigate(); const go = () => navigate({ to: "/register", search: { redirect } }); return null; }`,
    },
    // A lookup key is the allowlist; a conditional's test only picks a literal branch.
    {
      filename: "src/routes/login.tsx",
      code: `function R() { const search = useSearch(); const navigate = useNavigate(); navigate({ to: ROUTES[search.next] ?? "/" }); navigate({ to: search.tab === "a" ? "/a" : "/b" }); return null; }`,
    },
    { filename: "src/routes/login.tsx", code: `function R() { const { next } = useSearch(); return <Navigate to={SAFE[next] ?? "/home"} />; }` },
  ],
  invalid: [
    {
      filename: "src/routes/login.tsx",
      code: `function R() { const { redirect } = Route.useSearch(); const navigate = useNavigate(); navigate({ to: redirect }); return null; }`,
      errors: [{ messageId: "openRedirect" }],
    },
    {
      filename: "src/routes/login.tsx",
      code: `function R() { const search = useSearch(); return <Navigate to={search.next} />; }`,
      errors: [{ messageId: "openRedirect" }],
    },
    {
      filename: "routes/login.tsx",
      code: `function R() { const [params] = useSearchParams(); const navigate = useNavigate(); navigate(params.get("next") ?? "/", { replace: true }); return null; }`,
      errors: [{ messageId: "openRedirect" }],
    },
  ],
});

ruleTester.run("guard-tristate (route layouts)", plugin.rules["guard-tristate"], {
  valid: [{ filename: "src/routes/_app.tsx", code: `function G() { if (session.status === "anonymous") return <Navigate to="/login" />; return null; }` }],
  invalid: [
    {
      filename: "src/routes/_app.tsx",
      code: `function G() { if (!session.isAuthenticated) return <Navigate to="/login" />; return null; }`,
      errors: [{ messageId: "boolRedirect" }],
    },
  ],
});
