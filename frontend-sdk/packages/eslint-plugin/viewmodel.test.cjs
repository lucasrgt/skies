"use strict";

// The ViewModel rules (SKYFE004 render-agnostic, SKYFE007 mandatory state), the placeholder rule (SKYFE023), and the
// rule audit's precision fixes (SKYFE016's whole-word token keys, SKYFE017's redirect calls). Required by
// index.test.cjs.

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

// SKYFE004 — a ViewModel renders nothing and imports no react-dom.
ruleTester.run("viewmodel-render-agnostic", plugin.rules["viewmodel-render-agnostic"], {
  valid: [
    { filename: "src/features/items/Items.viewModel.ts", code: `import { useState } from "react"; export const useM = () => useState(0);` },
    // A type from react-dom (a portal container type) is erased at runtime.
    { filename: "src/features/items/Items.viewModel.ts", code: `import type { Container } from "react-dom/client";` },
    // The View renders; JSX there is the point.
    { filename: "src/features/items/Items.view.tsx", code: `import { createPortal } from "react-dom"; export const V = () => <p />;` },
  ],
  invalid: [
    {
      filename: "src/features/items/Items.viewModel.ts",
      code: `import { flushSync } from "react-dom";`,
      errors: [{ messageId: "dom" }],
    },
    {
      filename: "src/features/items/Items.viewModel.tsx",
      code: `export function useM() { return { banner: <p><b>hi</b></p>, empty: <></> }; }`,
      errors: [{ messageId: "jsx" }, { messageId: "jsx" }],
    },
  ],
});

// SKYFE007 — a query's data leaves the ViewModel as AsyncState.
ruleTester.run("mandatory-state", plugin.rules["mandatory-state"], {
  valid: [
    {
      filename: "src/features/items/Items.viewModel.ts",
      code: `import { toAsyncState } from "@skiesjs/react"; import { useListItems } from "@/client.gen/sample";
export function useM() { const q = useListItems(); return { items: toAsyncState({ isPending: q.isPending, isError: q.isError, data: q.data }) }; }`,
    },
    // A mutation's data is the command's success surface, not a resource (the sample's Deposit form).
    {
      filename: "src/features/deposit/Deposit.viewModel.ts",
      code: `import { useDeposit } from "@/client.gen/sample";
export function useM() { const m = useDeposit(); return { submit: () => m.mutate({}), completed: m.isSuccess ? m.data : null }; }`,
    },
    {
      filename: "src/features/deposit/Deposit.viewModel.ts",
      code: `import { useDeposit } from "@/client.gen/sample"; export function useM() { const { mutate, data } = useDeposit(); return { mutate, data }; }`,
    },
    // Hand-rolled explicit states: the pending and error flags travel with the data (seen 83 times in a real app).
    {
      filename: "src/features/requests/Requests.viewModel.ts",
      code: `import { useListRequests } from "@/client.gen/sample";
export function useM() { const q = useListRequests(); return { loading: q.isPending, error: q.isError, rows: q.data ?? [] }; }`,
    },
    // A secondary read beside a resource whose states are exposed: a form default hydrated from a cached query.
    {
      filename: "src/features/settings/Settings.viewModel.ts",
      code: `import { useLookupMe, useMe } from "@/client.gen/sample";
export function useM() { const q = useLookupMe(); const me = useMe(); return { loading: q.isPending, error: q.isError, host: q.data, email: me.data?.email }; }`,
    },
    // Not a ViewModel: the rule polices the data door only.
    { filename: "src/lib/session.ts", code: `import { useMe } from "@/client.gen/sample"; export const useS = () => useMe().data;` },
  ],
  invalid: [
    {
      filename: "src/features/items/Items.viewModel.ts",
      code: `import { useListItems } from "@/client.gen/sample"; export function useM() { const q = useListItems(); return { items: q.data }; }`,
      errors: [{ messageId: "raw", data: { hook: "useListItems" } }],
    },
    {
      filename: "src/features/items/Items.viewModel.ts",
      code: `import { useListItems } from "@/client.gen/sample"; export function useM() { const { data, isPending } = useListItems(); return { data, isPending }; }`,
      errors: [{ messageId: "raw" }],
    },
    {
      filename: "src/features/items/Items.viewModel.ts",
      code: `import { useGetItem } from "@/client.gen/sample"; export const useM = (id: string) => ({ item: useGetItem(id).data });`,
      errors: [{ messageId: "raw" }],
    },
    // Once per ViewModel, at the first read, like SKYFL007.
    {
      filename: "src/features/items/Items.viewModel.ts",
      code: `import { useA, useB } from "@/client.gen/sample"; export const useM = () => ({ a: useA().data, b: useB().data });`,
      errors: [{ messageId: "raw", data: { hook: "useA" } }],
    },
  ],
});

// SKYFE023 — no unfinished placeholder in production code (warning tier).
ruleTester.run("no-placeholder", plugin.rules["no-placeholder"], {
  valid: [
    { filename: "src/features/items/Items.viewModel.ts", code: `// Projects the query into the spine.\nexport const x = 1;` },
    // Tests, specs, and the generated client are not production code.
    { filename: "src/features/items/Items.test.ts", code: `// TODO: more cases\n// @ts-expect-error probing a bad input\nf(1);` },
    { filename: "app/.specs/0002-withdraw/e2e/withdraw.spec.ts", code: `// FIXME flaky on CI` },
    { filename: "src/client.gen/sample.ts", code: `// TODO generated` },
    // A word that merely contains a marker is not one.
    { filename: "src/lib/format.ts", code: `// Todos are listed by date; the hackathon badge renders separately.` },
    // Lowercase "todo" is a word: Portuguese "all" (calibration, a real app's comment).
    { filename: "src/ui/Button.tsx", code: `// Re-export puro pra preservar o import path em todo o app.` },
    { filename: "src/lib/errors.ts", code: `throw new Error("wallet not found");` },
  ],
  invalid: [
    { filename: "src/lib/a.ts", code: `// TODO wire the real endpoint`, errors: [{ messageId: "marker", data: { marker: "TODO" } }] },
    { filename: "src/lib/a.ts", code: `/* wire later */ export const x = 1;`, errors: [{ messageId: "marker" }] },
    {
      filename: "src/features/items/Items.viewModel.ts",
      code: `// @ts-expect-error the hook moved\nuseListItems(1);`,
      errors: [{ messageId: "silencer", data: { marker: "@ts-expect-error" } }],
    },
    { filename: "src/lib/a.ts", code: `export function f() { throw new Error("Not implemented"); }`, errors: [{ messageId: "stub" }] },
  ],
});

// SKYFE016 (audit) — a token-ish key is a whole word of the key: `author…` is not `auth`.
ruleTester.run("session-one-door (token keys)", plugin.rules["session-one-door"], {
  valid: [
    { filename: "src/features/profile/Profile.viewModel.ts", code: `localStorage.setItem("authorName", name);` },
    { filename: "src/features/profile/Profile.viewModel.ts", code: `localStorage.setItem("authority", value);` },
    { filename: "src/features/profile/Profile.viewModel.ts", code: `sessionStorage.setItem("draft-author", value);` },
  ],
  invalid: [
    { filename: "src/features/a/A.viewModel.ts", code: `localStorage.setItem("auth_token", t);`, errors: [{ messageId: "storage" }] },
    { filename: "src/features/a/A.viewModel.ts", code: `window.sessionStorage.setItem("app.session", s);`, errors: [{ messageId: "storage" }] },
    { filename: "src/features/a/A.viewModel.ts", code: `localStorage.setItem("jwt", s);`, errors: [{ messageId: "storage" }] },
  ],
});

// SKYFE017 (twin alignment) — a redirect decided on an auth boolean, spelled as TanStack's `throw redirect(…)` or a
// React Router loader's `return redirect(…)` as well as `return <Navigate/>`.
ruleTester.run("guard-tristate (redirect calls)", plugin.rules["guard-tristate"], {
  valid: [
    {
      filename: "src/routes/_authed.tsx",
      code: `export const Route = { beforeLoad: ({ context }) => { if (context.session.status === "anonymous") throw redirect({ to: "/login" }); } };`,
    },
  ],
  invalid: [
    {
      filename: "src/routes/_authed.tsx",
      code: `export const Route = { beforeLoad: ({ context }) => { if (!context.auth.isAuthenticated) throw redirect({ to: "/login" }); } };`,
      errors: [{ messageId: "boolRedirect" }],
    },
    {
      filename: "app/routes/account.tsx",
      code: `export async function loader() { if (!isLoggedIn) { return redirect("/login"); } return null; }`,
      errors: [{ messageId: "boolRedirect" }],
    },
  ],
});
