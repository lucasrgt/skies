"use strict";

// Self-test for the SKYFE rules. Every rule is pinned here with RuleTester: it must FIRE on the violation it polices
// and PASS on the shapes it allows. Run: `node index.test.cjs` (exits non-zero on any failing case). eslint + the TS
// parser are workspace devDependencies, so a plain require resolves them from the hoisted node_modules.

const { RuleTester } = require("eslint");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const tsParser = require("@typescript-eslint/parser");
const plugin = require("./index.cjs");
const manifest = require("./package.json");

assert.equal(plugin.meta.version, manifest.version, "plugin metadata must match its package version");

// The rule ids are public API (apps reference them in their eslint config) — pin the set, and keep one file per rule.
const RULE_IDS = [
  "view-purity",
  "data-door",
  "no-mock",
  "viewmodel-platform-agnostic",
  "state-completeness",
  "i18n-completeness",
  "mutation-error-handled",
  "no-hardcoded-copy",
  "no-router-replace-in-effect",
  "session-one-door",
  "guard-tristate",
  "route-param-guard",
  "safe-back",
  "no-hardcoded-base-url",
  "no-raw-html",
  "no-open-redirect",
  "query-client-defaults",
  "no-manual-refetch-ritual",
  "refresh-one-door",
  "no-cast-navigation",
  "submit-handles-invalid",
  "controller-field-state",
  "tests-live-in-specs",
];
assert.deepEqual(Object.keys(plugin.rules).sort(), [...RULE_IDS].sort(), "rule ids are stable");
assert.deepEqual(
  fs.readdirSync(path.join(__dirname, "rules")).sort(),
  RULE_IDS.map((id) => `${id}.cjs`).sort(),
  "one file per rule under rules/",
);
assert.equal(plugin.configs.recommended, plugin.configs["flat/recommended"]);
assert.deepEqual(
  Object.keys(plugin.configs.recommended.rules).sort(),
  RULE_IDS.map((id) => `skies/${id}`).sort(),
  "recommended enables every rule",
);

const ruleTester = new RuleTester({
  languageOptions: {
    parser: tsParser,
    ecmaVersion: 2022,
    sourceType: "module",
    parserOptions: { ecmaFeatures: { jsx: true } },
  },
});

// SKYFE001 — a View (*.view.tsx) imports no data layer (generated client / axios / react-query); it consumes its
// ViewModel.
ruleTester.run("view-purity", plugin.rules["view-purity"], {
  valid: [
    // A View may import the contract TYPES (erased at runtime) and the ViewModel.
    { filename: "Foo.view.tsx", code: `import type { Thing } from "@/client.gen/model";` },
    { filename: "Foo.view.tsx", code: `import { useFooModel } from "./Foo.viewModel";` },
    // Non-views are out of this rule's scope.
    { filename: "Foo.viewModel.ts", code: `import axios from "axios";` },
  ],
  invalid: [
    { filename: "Foo.view.tsx", code: `import { useThing } from "@/client.gen/sample";`, errors: [{ messageId: "impure" }] },
    { filename: "Foo.view.tsx", code: `import axios from "axios";`, errors: [{ messageId: "impure" }] },
    { filename: "Foo.view.tsx", code: `import { useQuery } from "@tanstack/react-query";`, errors: [{ messageId: "impure" }] },
  ],
});

// SKYFE002 — the generated client has two data doors: a screen's *.viewModel.ts, and the framework auth/routing
// infra (lib/session, lib/guards). Everything else is forbidden; type-only imports are always fine.
ruleTester.run("data-door", plugin.rules["data-door"], {
  valid: [
    { filename: "Foo.viewModel.ts", code: `import { useThing } from "@/client.gen/sample";` },
    { filename: "src/lib/session/session.ts", code: `import { refresh } from "@/client.gen/sample";` },
    // The seam may be a single FILE, not only a directory (lib/session.ts) — both are the auth/routing infra door.
    { filename: "src/lib/session.ts", code: `import { useMe, getMeQueryKey } from "@/client.gen/sample";` },
    { filename: "src/lib/guards/RouteGuard.tsx", code: `import { useMe } from "@/client.gen/sample";` },
    { filename: "Foo.view.tsx", code: `import type { Thing } from "@/client.gen/model";` },
    // Generated enums are contract vocabulary, not a server-access door.
    { filename: "Foo.view.tsx", code: `import { Status } from "@/client.gen/model";` },
  ],
  invalid: [
    { filename: "Foo.view.tsx", code: `import { useThing } from "@/client.gen/sample";`, errors: [{ messageId: "offdoor" }] },
    { filename: "src/app/_layout.tsx", code: `import { refresh } from "@/client.gen/sample";`, errors: [{ messageId: "offdoor" }] },
    { filename: "src/lib/skies-client.ts", code: `import { thing } from "@/client.gen/sample";`, errors: [{ messageId: "offdoor" }] },
  ],
});

// SKYFE009 — a *.viewModel.ts is platform-agnostic: no react-native / expo import (value OR type).
ruleTester.run("viewmodel-platform-agnostic", plugin.rules["viewmodel-platform-agnostic"], {
  valid: [
    { filename: "Foo.viewModel.ts", code: `import { useState } from "react";` },
    { filename: "Foo.viewModel.ts", code: `import { useThing } from "@/client.gen/sample";` },
    // A View may import react-native — it is the platform layer.
    { filename: "Foo.view.tsx", code: `import { View } from "react-native";` },
  ],
  invalid: [
    { filename: "Foo.viewModel.ts", code: `import { Platform } from "react-native";`, errors: [{ messageId: "platform" }] },
    { filename: "Foo.viewModel.ts", code: `import * as SecureStore from "expo-secure-store";`, errors: [{ messageId: "platform" }] },
    { filename: "Foo.viewModel.ts", code: `import type { ViewProps } from "react-native";`, errors: [{ messageId: "platform" }] },
  ],
});

// SKYFE003 — no mock / fixture / MSW import in production code (only under *.test.*).
ruleTester.run("no-mock", plugin.rules["no-mock"], {
  valid: [
    { filename: "Foo.test.tsx", code: `import { server } from "msw";` },
    { filename: "Foo.viewModel.ts", code: `import { useState } from "react";` },
  ],
  invalid: [
    { filename: "Foo.viewModel.ts", code: `import { server } from "msw";`, errors: [{ messageId: "mock" }] },
    { filename: "Foo.view.tsx", code: `import { fake } from "./__mocks__/thing";`, errors: [{ messageId: "mock" }] },
  ],
});

// SKYFE010 — a View routes async state through <Resource>, never raw react-query booleans (member access OR
// destructuring). The booleans are the ViewModel's; the View consumes the AsyncState union.
ruleTester.run("state-completeness", plugin.rules["state-completeness"], {
  valid: [
    { filename: "Foo.view.tsx", code: `const { state } = useFooModel(); const data = state.items;` },
    // Non-views own the booleans (the ViewModel projects them through toAsyncState).
    { filename: "Foo.viewModel.ts", code: `const x = query.isPending;` },
  ],
  invalid: [
    { filename: "Foo.view.tsx", code: `const x = query.isPending;`, errors: [{ messageId: "raw" }] },
    { filename: "Foo.view.tsx", code: `const { isError } = useFooModel();`, errors: [{ messageId: "raw" }] },
  ],
});

// SKYFE011 — every locale catalog in a *.i18n.ts declares the same keys; a key in one but not its siblings is a
// silent untranslated string. Scope is the *.i18n.ts file only.
ruleTester.run("i18n-completeness", plugin.rules["i18n-completeness"], {
  valid: [
    { filename: "x.i18n.ts", code: `export const ptBR = { a: "1", b: "2" }; export const enUS = { a: "x", b: "y" };` },
    // `as const` is unwrapped; dotted keys compared as flat keys.
    {
      filename: "x.i18n.ts",
      code: `export const ptBR = { "e.t": "1" } as const; export const enUS = { "e.t": "x" } as const;`,
    },
    // A single catalog has nothing to compare against.
    { filename: "x.i18n.ts", code: `export const ptBR = { a: "1" };` },
  ],
  invalid: [
    {
      filename: "x.i18n.ts",
      code: `export const ptBR = { a: "1", b: "2" }; export const enUS = { a: "x" };`,
      errors: [{ messageId: "missing" }],
    },
  ],
});

// SKYFE013 — a ViewModel's mutation must surface its failure (no silent failure). Scoped to *.viewModel.ts. Four
// legitimate surfaces are accepted: (A) inline onError, (B) a read `.isError` state, (C) mutateAsync in try/catch
// or .catch(), (D) a returned/propagated mutateAsync. Each is a real surface; demanding a redundant onError on top
// would be the very test-theater the rule exists to prevent.
ruleTester.run("mutation-error-handled", plugin.rules["mutation-error-handled"], {
  valid: [
    // A) inline onError (also via a spread that may carry it).
    { filename: "Foo.viewModel.ts", code: `m.mutate(data, { onSuccess: ok, onError: fail });` },
    { filename: "Foo.viewModel.ts", code: `m.mutateAsync(data, { onError: fail });` },
    { filename: "Foo.viewModel.ts", code: `m.mutate(data, { ...handlers });` },
    // B) the mutation handle's .isError is read elsewhere in the file (surfaced as state the View renders).
    { filename: "Foo.viewModel.ts", code: `saveMut.mutate(data, { onSuccess: ok }); const e = saveMut.isError ? "x" : null;` },
    // C) mutateAsync inside a try/catch, or chained with .catch().
    { filename: "Foo.viewModel.ts", code: `async function f(){ try { await m.mutateAsync(data); } catch { setErr(); } }` },
    { filename: "Foo.viewModel.ts", code: `m.mutateAsync(data).catch(() => undefined);` },
    // D) a thin wrapper that returns the mutateAsync promise — propagates to the awaiting caller.
    { filename: "Foo.viewModel.ts", code: `const run = () => m.mutateAsync(data);` },
    { filename: "Foo.viewModel.ts", code: `function run(){ return m.mutateAsync(data); }` },
    // Out of scope: a .mutate outside a ViewModel isn't this rule's concern.
    { filename: "Foo.view.tsx", code: `m.mutate(data);` },
    // Not a react-query mutation call shape.
    { filename: "Foo.viewModel.ts", code: `arr.map(x => x);` },
  ],
  invalid: [
    { filename: "Foo.viewModel.ts", code: `m.mutate(data);`, errors: [{ messageId: "unhandled" }] },
    { filename: "Foo.viewModel.ts", code: `m.mutate(data, { onSuccess: ok });`, errors: [{ messageId: "unhandled" }] },
    { filename: "Foo.viewModel.ts", code: `m.mutateAsync(data, {});`, errors: [{ messageId: "unhandled" }] },
    // isPending read is not error handling — only isError/error/failureReason count.
    { filename: "Foo.viewModel.ts", code: `m.mutate(data); const p = m.isPending;`, errors: [{ messageId: "unhandled" }] },
    // a bare awaited mutateAsync (no try/catch, no .catch, not returned) is still a silent failure.
    { filename: "Foo.viewModel.ts", code: `async function f(){ await m.mutateAsync(data); }`, errors: [{ messageId: "unhandled" }] },
  ],
});

// SKYFE014 — no hardcoded user-facing copy in a View (JSX text children must go through t()).
ruleTester.run("no-hardcoded-copy", plugin.rules["no-hardcoded-copy"], {
  valid: [
    // t() result is an expression, not text — never flagged.
    { filename: "Foo.view.tsx", code: `const x = <Text>{t("k")}</Text>;` },
    // attributes (className/testID) are not text children.
    { filename: "Foo.view.tsx", code: `const x = <View className="flex-1" testID="x" />;` },
    // whitespace / non-letter text is ignored.
    { filename: "Foo.view.tsx", code: `const x = <Text> </Text>;` },
    { filename: "Foo.view.tsx", code: `const x = <Text>{count}</Text>;` },
    // Phase 2 props: t() / variables on copy props are expressions, not literals — not flagged.
    { filename: "Foo.view.tsx", code: `const x = <Input placeholder={t("k")} />;` },
    { filename: "Foo.view.tsx", code: `const x = <Input placeholder={ph} />;` },
    // non-copy props with literals are fine (name/testID/variant aren't copy).
    { filename: "Foo.view.tsx", code: `const x = <Icon name="check" testID="x" variant="primary" />;` },
    // out of scope: not a *.view.tsx
    { filename: "Foo.tsx", code: `const x = <Text>Entrar</Text>;` },
  ],
  invalid: [
    { filename: "Foo.view.tsx", code: `const x = <Text>Entrar na conta</Text>;`, errors: [{ messageId: "hardcoded" }] },
    { filename: "Foo.view.tsx", code: `const x = <Button>Salvar</Button>;`, errors: [{ messageId: "hardcoded" }] },
    // Phase 2: hardcoded copy in a copy-bearing prop.
    { filename: "Foo.view.tsx", code: `const x = <Input placeholder="Seu e-mail" />;`, errors: [{ messageId: "hardcoded" }] },
    { filename: "Foo.view.tsx", code: `const x = <EmptyState title="Nada aqui" />;`, errors: [{ messageId: "hardcoded" }] },
  ],
});

// SKYFE015 — no imperative redirect inside useEffect (a guard redirect must be declarative <Redirect>/<Navigate>, not
// an effect that loops on web). Recognizes both router idioms; user-action push/back and onPress redirect stay allowed.
ruleTester.run("no-router-replace-in-effect", plugin.rules["no-router-replace-in-effect"], {
  valid: [
    // Declarative redirect — the correct shape (both idioms).
    { filename: "Foo.view.tsx", code: `export const F = () => (done ? <Redirect href="/home" /> : null);` },
    { filename: "src/app/x.tsx", code: `export const F = () => (done ? <Navigate to="/home" /> : null);` },
    // Imperative replace on a user action (not in an effect) — a navigation completion, allowed.
    { filename: "Foo.view.tsx", code: `export const F = () => { const onPress = () => router.replace("/home"); return null; };` },
    // A TanStack navigate() on a user action is allowed.
    { filename: "src/app/x.tsx", code: `export const F = () => { const navigate = useNavigate(); const onPress = () => navigate({ to: "/home" }); return null; };` },
    // push/back inside an effect are not the banned call.
    { filename: "Foo.view.tsx", code: `export const F = () => { useEffect(() => { router.push("/home"); }, []); };` },
    // out of scope: not a View or route.
    { filename: "Foo.viewModel.ts", code: `useEffect(() => { router.replace("/home"); }, []);` },
  ],
  invalid: [
    // expo-router: router.replace / router.navigate in an effect.
    { filename: "Foo.view.tsx", code: `export const F = () => { useEffect(() => { router.replace("/home"); }, []); };`, errors: [{ messageId: "effectReplace" }] },
    { filename: "Foo.view.tsx", code: `export const F = () => { useLayoutEffect(() => { if (x) router.navigate("/x"); }, [x]); };`, errors: [{ messageId: "effectReplace" }] },
    // TanStack: a useNavigate() binding called in an effect.
    { filename: "src/app/x.tsx", code: `export const F = () => { const navigate = useNavigate(); useEffect(() => { navigate({ to: "/home" }); }, []); };`, errors: [{ messageId: "effectReplace" }] },
  ],
});

// SKYFE016 — the session token is written through one seam (lib/session); a viewModel/view importing the token setter
// directly is the scattered-write bug that forgets the cache reset.
ruleTester.run("session-one-door", plugin.rules["session-one-door"], {
  valid: [
    // The seam itself legitimately imports the setter (it pairs the write with the reset) — directory or file form.
    { filename: "src/lib/session/session.ts", code: `import { setAccessToken } from "@/lib/skies-client";` },
    { filename: "src/lib/session.ts", code: `import { setAccessToken } from "@/lib/skies-client";` },
    // A viewModel going through the seam is correct.
    { filename: "Login.viewModel.ts", code: `import { useSignIn } from "@/lib/session";` },
    // The client that DEFINES the setter exports it — it does not import it, so it is never flagged.
    { filename: "src/lib/skies-client.ts", code: `export function setAccessToken(t) {}` },
  ],
  invalid: [
    { filename: "Login.viewModel.ts", code: `import { setAccessToken } from "@/lib/skies-client";`, errors: [{ messageId: "offdoor" }] },
    { filename: "SignupWizard.viewModel.ts", code: `import { setToken } from "@/lib/skies-client";`, errors: [{ messageId: "offdoor" }] },
  ],
});

// SKYFE017 — a guard redirects on a tri-state SessionState, not a raw isAuthenticated boolean.
ruleTester.run("guard-tristate", plugin.rules["guard-tristate"], {
  valid: [
    // Branch on the union — loading is a distinct, handled case.
    { filename: "src/app/routes/index.tsx", code: `function H() { if (session.status === "anonymous") return <Navigate to="/login" />; return null; }` },
    // A non-auth presence guard (a param) is SKYFE018's domain, not this rule's.
    { filename: "src/app/routes/index.tsx", code: `function H() { if (!chatId) return <Redirect href="/m" />; return null; }` },
    // out of scope: a plain feature view is not a route/guard.
    { filename: "Foo.view.tsx", code: `function H() { if (!isAuthenticated) return <Navigate to="/login" />; return null; }` },
  ],
  invalid: [
    { filename: "src/app/routes/index.tsx", code: `function H() { if (!session.isAuthenticated) return <Navigate to="/login" />; return null; }`, errors: [{ messageId: "boolRedirect" }] },
    { filename: "src/lib/guards/Admin.tsx", code: `function H() { if (!isAuthenticated) return <Redirect href="/login" />; return null; }`, errors: [{ messageId: "boolRedirect" }] },
  ],
});

// SKYFE018 — a route reading a required id param must guard its absence with a declarative redirect.
ruleTester.run("route-param-guard", plugin.rules["route-param-guard"], {
  valid: [
    // The param is guarded before the View renders.
    { filename: "src/app/messaging/chat.tsx", code: `function R() { const { chatId } = useLocalSearchParams(); if (!chatId) return <Redirect href="/messaging" />; return <Chat chatId={chatId} />; }` },
    // Guarding a COALESCED value covers every param feeding it — `const id = a ?? b; if (!id) …` is not flagged.
    { filename: "src/app/x.tsx", code: `function R() { const { propertyId, id } = useLocalSearchParams(); const resolved = propertyId ?? id; if (!resolved) return <Redirect href="/" />; return <V propertyId={resolved} />; }` },
    // Guarding a renamed param covers it.
    { filename: "src/app/y.tsx", code: `function R() { const { id } = useLocalSearchParams(); const propertyId = id; if (!propertyId) return <Redirect href="/" />; return <V propertyId={propertyId} />; }` },
    // Two required params guarded together by a `||`-chain — both covered (redirects if either is missing).
    { filename: "src/app/z.tsx", code: `function R() { const { id, propertyId } = useLocalSearchParams(); if (!id || !propertyId) return <Redirect href="/" />; return <V id={id} propertyId={propertyId} />; }` },
    // The spine's union shape: requiredParam() + a status === "missing" branch guards the param.
    { filename: "src/app/chat.tsx", code: `function R() { const { chatId } = useLocalSearchParams(); const id = requiredParam(chatId); if (id.status === "missing") return <Redirect href="/messaging" />; return <Chat chatId={id.value} />; }` },
    // A non-id param (an optional filter) does not ghost — not required.
    { filename: "src/app/list.tsx", code: `function R() { const { tab } = useLocalSearchParams(); return <List tab={tab} />; }` },
    // out of scope: not a route file.
    { filename: "Foo.view.tsx", code: `function R() { const { chatId } = useLocalSearchParams(); return <Chat chatId={chatId} />; }` },
  ],
  invalid: [
    { filename: "src/app/messaging/chat.tsx", code: `function R() { const { chatId } = useLocalSearchParams(); return <Chat chatId={chatId} />; }`, errors: [{ messageId: "unguarded" }] },
    { filename: "src/app/property/detail.tsx", code: `function R() { const { id } = useLocalSearchParams(); return <Detail id={id} />; }`, errors: [{ messageId: "unguarded" }] },
  ],
});

// SKYFE019 — no bare router.back() / history.back(); route Back through a guarded helper.
ruleTester.run("safe-back", plugin.rules["safe-back"], {
  valid: [
    // The guarded helper — the correct shape.
    { filename: "Header.view.tsx", code: `const onBack = useGoBack("/");` },
    // An inline canGoBack-guarded back() is fine (the file references canGoBack).
    { filename: "Header.view.tsx", code: `const onBack = () => { if (router.canGoBack()) router.back(); else router.replace("/"); };` },
    // The nav seam (where the helper lives) is exempt.
    { filename: "src/lib/useGoBack.ts", code: `export const useGoBack = () => () => router.back();` },
    // push is not the banned call.
    { filename: "Header.view.tsx", code: `const onNext = () => router.push("/x");` },
  ],
  invalid: [
    { filename: "Header.view.tsx", code: `const onBack = () => router.back();`, errors: [{ messageId: "bareBack" }] },
    { filename: "src/app/notifications.tsx", code: `const onBack = () => window.history.back();`, errors: [{ messageId: "bareBack" }] },
  ],
});

// SKYFE020 — the API base URL comes from configuration, not a hardcoded host baked into the client's construction.
ruleTester.run("no-hardcoded-base-url", plugin.rules["no-hardcoded-base-url"], {
  valid: [
    // env-driven with a relative fallback (web) — the blessed shape.
    { filename: "src/lib/skies-client.ts", code: `const c = axios.create({ baseURL: import.meta.env.VITE_API_URL ?? "" });` },
    // env-driven with an env fallback (mobile).
    { filename: "src/lib/skies-client.ts", code: `const c = axios.create({ baseURL: process.env.EXPO_PUBLIC_API_URL ?? "http://localhost:8080" });` },
    // a relative base is configuration, not a baked host.
    { filename: "src/lib/skies-client.ts", code: `const c = axios.create({ baseURL: "/api" });` },
    // an injectable default overridden at boot (the hostpoint pattern) is an assignment, not a construction prop.
    { filename: "src/lib/skies-client.ts", code: `instance.defaults.baseURL = "http://localhost:8080";` },
    // out of scope: a test fixture may hardcode a URL.
    { filename: "Foo.test.tsx", code: `const c = axios.create({ baseURL: "http://localhost:8080" });` },
  ],
  invalid: [
    { filename: "src/lib/skies-client.ts", code: `const c = axios.create({ baseURL: "http://localhost:8080" });`, errors: [{ messageId: "hardcoded" }] },
    { filename: "src/lib/client.ts", code: `const c = makeClient({ baseUrl: "https://api.example.com" });`, errors: [{ messageId: "hardcoded" }] },
  ],
});

// SKYFE021 — no dangerouslySetInnerHTML outside the lib/html seam (the one audited, sanitizing door).
ruleTester.run("no-raw-html", plugin.rules["no-raw-html"], {
  valid: [
    // The seam owns rich-HTML rendering — the sanitizer is wired there.
    { filename: "src/lib/html/RichText.tsx", code: `const x = <div dangerouslySetInnerHTML={{ __html: clean }} />;` },
    // Plain JSX text is escaped by construction.
    { filename: "Foo.view.tsx", code: `const x = <div>{body}</div>;` },
    // A test may exercise the seam.
    { filename: "Foo.test.tsx", code: `const x = <div dangerouslySetInnerHTML={{ __html: "<b>x</b>" }} />;` },
  ],
  invalid: [
    { filename: "Foo.view.tsx", code: `const x = <div dangerouslySetInnerHTML={{ __html: body }} />;`, errors: [{ messageId: "rawHtml" }] },
    { filename: "src/app/post.tsx", code: `const x = <article dangerouslySetInnerHTML={{ __html: post.html }} />;`, errors: [{ messageId: "rawHtml" }] },
  ],
});

// SKYFE022 — never navigate to a value that arrived in the URL (open redirect); map it through an allowlist first.
ruleTester.run("no-open-redirect", plugin.rules["no-open-redirect"], {
  valid: [
    // Navigating to a literal route is fine.
    { filename: "src/app/login.tsx", code: `function R() { const { returnTo } = useLocalSearchParams(); router.replace("/home"); return null; }` },
    // The allowlisted mapping is the blessed shape: the raw param picks a KNOWN route, never becomes one.
    { filename: "src/app/login.tsx", code: `function R() { const { returnTo } = useLocalSearchParams(); const to = ROUTES.has(returnTo) ? returnTo : "/home"; return null; }` },
    // out of scope: a viewModel does not navigate.
    { filename: "Foo.viewModel.ts", code: `const { returnTo } = useLocalSearchParams(); router.replace(returnTo);` },
  ],
  invalid: [
    { filename: "src/app/login.tsx", code: `function R() { const { returnTo } = useLocalSearchParams(); router.replace(returnTo); return null; }`, errors: [{ messageId: "openRedirect" }] },
    { filename: "src/app/login.tsx", code: `function R() { const params = useLocalSearchParams(); router.push(params.next); return null; }`, errors: [{ messageId: "openRedirect" }] },
    { filename: "src/app/login.tsx", code: `function R() { const { next } = useLocalSearchParams(); window.location.href = next; return null; }`, errors: [{ messageId: "openRedirect" }] },
    { filename: "Foo.view.tsx", code: `function R() { const { next } = useSearch(); location.assign(next); return null; }`, errors: [{ messageId: "openRedirect" }] },
  ],
});

// SKYFE016 (storage half) — a token-ish storage write outside the seam is the same scattered session write as
// importing the setter; only lib/session touches token storage.
ruleTester.run("session-one-door", plugin.rules["session-one-door"], {
  valid: [
    // The seam owns the storage write.
    { filename: "src/lib/session.ts", code: `localStorage.setItem("accessToken", t);` },
    // A non-token key is app state, not a session write.
    { filename: "Settings.viewModel.ts", code: `localStorage.setItem("theme", "dark");` },
    // Tests seed storage freely.
    { filename: "Login.test.tsx", code: `localStorage.setItem("accessToken", "fake");` },
  ],
  invalid: [
    { filename: "Login.viewModel.ts", code: `localStorage.setItem("accessToken", t);`, errors: [{ messageId: "storage" }] },
    { filename: "Login.viewModel.ts", code: `window.localStorage.setItem("auth.session", s);`, errors: [{ messageId: "storage" }] },
    { filename: "Login.viewModel.ts", code: `SecureStore.setItemAsync("jwt", t);`, errors: [{ messageId: "storage" }] },
  ],
});

// SKYFE002 (re-export half) — `export … from "client.gen"` outside the doors launders the client to every importer.
ruleTester.run("data-door", plugin.rules["data-door"], {
  valid: [
    // Type re-exports are the shared contract vocabulary, not data access.
    { filename: "src/lib/contracts.ts", code: `export type { Thing } from "@/client.gen/model";` },
    { filename: "src/lib/contracts.ts", code: `export { Status } from "@/client.gen/model";` },
    // The ViewModel door may compose over the client however it likes.
    { filename: "Foo.viewModel.ts", code: `export { useThing } from "@/client.gen/sample";` },
  ],
  invalid: [
    { filename: "src/lib/api.ts", code: `export { useThing } from "@/client.gen/sample";`, errors: [{ messageId: "laundered" }] },
    { filename: "src/lib/api.ts", code: `export * from "@/client.gen/sample";`, errors: [{ messageId: "laundered" }] },
  ],
});

// SKYFE013 (empty-handler half) — `onError: () => {}` is the silent failure with paperwork; the structure must
// route the error somewhere, not merely exist.
ruleTester.run("mutation-error-handled", plugin.rules["mutation-error-handled"], {
  valid: [
    // A handler with a body is trusted (its quality is human judgment, not lint's).
    { filename: "Foo.viewModel.ts", code: `mut.mutate(input, { onError: (e) => setError(e) });` },
  ],
  invalid: [
    { filename: "Foo.viewModel.ts", code: `mut.mutate(input, { onError: () => {} });`, errors: [{ messageId: "empty" }] },
  ],
});

// SKYFE011 (nested half) — keys are compared as flattened paths, so a key missing inside a nested group is caught.
ruleTester.run("i18n-completeness", plugin.rules["i18n-completeness"], {
  valid: [
    {
      filename: "x.i18n.ts",
      code: `export const ptBR = { empty: { title: "1" } }; export const enUS = { empty: { title: "x" } };`,
    },
  ],
  invalid: [
    {
      filename: "x.i18n.ts",
      code: `export const ptBR = { empty: { title: "1", hint: "2" } }; export const enUS = { empty: { title: "x" } };`,
      errors: [{ messageId: "missing" }],
    },
  ],
});

// SKYFE027 — a QueryClient carries the mutation defaults (mutationCache: new MutationCache({ onSuccess, onError })).
// Tests and the shared test harness construct bare clients freely; an inline or same-file-declared cache is checked,
// anything built further away is trusted as visible-in-review.
ruleTester.run("query-client-defaults", plugin.rules["query-client-defaults"], {
  valid: [
    // The blessed shape — inline cache with both handlers.
    {
      filename: "src/lib/query.ts",
      code: `const qc = new QueryClient({ mutationCache: new MutationCache({ onSuccess: inv, onError: feed }) });`,
    },
    // The cache declared first, referenced by name — still checked, still conformant.
    {
      filename: "src/lib/query.ts",
      code: `const cache = new MutationCache({ onSuccess: inv, onError: feed }); const qc = new QueryClient({ mutationCache: cache });`,
    },
    // A spread may carry the handlers (or the cache) — trusted.
    { filename: "src/lib/query.ts", code: `const qc = new QueryClient({ mutationCache: new MutationCache({ ...defaults }) });` },
    { filename: "src/lib/query.ts", code: `const qc = new QueryClient({ ...base });` },
    // An options factory is visible in review — not a false positive.
    { filename: "src/lib/query.ts", code: `const qc = new QueryClient(makeOptions());` },
    // Tests and the shared test harness build throwaway clients for ISOLATION — the defaults are deliberately absent.
    { filename: "Foo.test.tsx", code: `const c = new QueryClient();` },
    { filename: "src/test/providers.tsx", code: `const c = new QueryClient({ defaultOptions: { queries: { retry: false } } });` },
  ],
  invalid: [
    // The pilot's exact bug: a bare client — no invalidation, no feedback, 13 ViewModels left to remember by hand.
    { filename: "src/lib/queryClient.ts", code: `export const queryClient = new QueryClient();`, errors: [{ messageId: "missing" }] },
    {
      filename: "src/lib/query.ts",
      code: `const qc = new QueryClient({ defaultOptions: { queries: { retry: 1 } } });`,
      errors: [{ messageId: "missing" }],
    },
    // A cache without both handlers is the defaults half-wired.
    {
      filename: "src/lib/query.ts",
      code: `const qc = new QueryClient({ mutationCache: new MutationCache({ onSuccess: inv }) });`,
      errors: [{ messageId: "incomplete" }],
    },
    {
      filename: "src/lib/query.ts",
      code: `const qc = new QueryClient({ mutationCache: new MutationCache() });`,
      errors: [{ messageId: "incomplete" }],
    },
    {
      filename: "src/lib/query.ts",
      code: `const cache = new MutationCache({ onError: feed }); const qc = new QueryClient({ mutationCache: cache });`,
      errors: [{ messageId: "incomplete" }],
    },
  ],
});

// SKYFE028 — an onSuccess whose entire body is refetch/invalidate calls duplicates the SKYFE027 defaults (the pilot's
// hand-rolled ritual, 30 of 43 ViewModels). A handler that does MORE than refetch is real behavior — never flagged.
ruleTester.run("no-manual-refetch-ritual", plugin.rules["no-manual-refetch-ritual"], {
  valid: [
    // Does more than refetch — navigation/handoff is behavior, not ritual.
    { filename: "Foo.viewModel.ts", code: `useCreateThing({ mutation: { onSuccess: (r) => onCreated(r.id) } });` },
    // Mixed body: the refetch may be redundant, the rest is not — human judgment, not lint's.
    {
      filename: "Foo.viewModel.ts",
      code: `const refetch = () => void q.refetch(); m.mutate(d, { onSuccess: () => { refetch(); reset(); } });`,
    },
    // An unresolvable name is not assumed to be a ritual.
    { filename: "Foo.viewModel.ts", code: `m.mutate(d, { onSuccess: props.onSaved });` },
    // out of scope: only the ViewModel owns mutations.
    { filename: "Foo.view.tsx", code: `m.mutate(d, { onSuccess: () => q.refetch() });` },
  ],
  invalid: [
    // The pilot's exact shapes: a named ritual passed to the hook options…
    {
      filename: "Foo.viewModel.ts",
      code: `const refetch = () => void departments.refetch(); const create = useCreateDepartment({ mutation: { onSuccess: refetch } });`,
      errors: [{ messageId: "ritual" }],
    },
    // …an inline arrow at the call-site…
    { filename: "Foo.viewModel.ts", code: `m.mutate(d, { onSuccess: () => list.refetch() });`, errors: [{ messageId: "ritual" }] },
    // …and the useCallback-wrapped invalidate (one level of indirection, still pure).
    {
      filename: "Foo.viewModel.ts",
      code: `const invalidate = useCallback(() => { queryClient.invalidateQueries({ queryKey: k }); }, [queryClient]); m.mutate(d, { onSuccess: invalidate });`,
      errors: [{ messageId: "ritual" }],
    },
    {
      filename: "Foo.viewModel.ts",
      code: `const invalidateSteps = useCallback(() => queryClient.invalidateQueries({ queryKey: k }), [queryClient]); const up = useUpdateStep({ mutation: { onSuccess: () => void invalidateSteps() } });`,
      errors: [{ messageId: "ritual" }],
    },
  ],
});

// SKYFE013 ({ globalSurface: true } half) — with the SKYFE027 defaults wired, the global MutationCache.onError IS the
// surface (react-query fires it regardless of per-call handlers), so a bare .mutate() passes; the empty onError stays
// flagged — it is dead paperwork either way.
ruleTester.run("mutation-error-handled", plugin.rules["mutation-error-handled"], {
  valid: [
    { filename: "Foo.viewModel.ts", code: `m.mutate(data);`, options: [{ globalSurface: true }] },
    { filename: "Foo.viewModel.ts", code: `m.mutate(data, { onSuccess: ok });`, options: [{ globalSurface: true }] },
    { filename: "Foo.viewModel.ts", code: `async function f(){ await m.mutateAsync(data); }`, options: [{ globalSurface: true }] },
  ],
  invalid: [
    {
      filename: "Foo.viewModel.ts",
      code: `m.mutate(data, { onError: () => {} });`,
      options: [{ globalSurface: true }],
      errors: [{ messageId: "empty" }],
    },
  ],
});

// SKYFE029 — refresh-one-door: the session rotation has exactly one consumer surface (the client seam's
// single-flight interceptor / the session seam's gated bootstrap). A second consumer — the refresh hook/op
// imported into a screen, or a hand-rolled POST to a refresh route — eventually rotates in parallel and the
// backend's theft detection burns the session family.
ruleTester.run("refresh-one-door", plugin.rules["refresh-one-door"], {
  valid: [
    // The session seam may consume the rotation (a gated boot bootstrap composes the client's single-flight).
    { filename: "src/lib/session.ts", code: `import { refreshAccessToken } from "@/lib/skies-client";` },
    { filename: "src/lib/session/useSession.ts", code: `import { refresh } from "@/client.gen/sample";` },
    // The client seam itself defines the rotation — its own raw post is the door's inside.
    { filename: "src/lib/skies-client.ts", code: `instance.post("/account/refresh", {});` },
    // Type-only imports are contract vocabulary, not a rotation path.
    { filename: "Foo.viewModel.ts", code: `import type { refresh } from "@/client.gen/sample";` },
    // Unrelated names from the client are fine.
    { filename: "Foo.viewModel.ts", code: `import { useLogin } from "@/client.gen/sample";` },
    // A refresh-named import from a NON-client source is not the rotation (e.g. a UI helper).
    { filename: "Foo.viewModel.ts", code: `import { refresh } from "@/lib/animation";` },
    // Tests exercise freely.
    { filename: "Session.test.tsx", code: `import { refreshAccessToken } from "@/lib/skies-client";` },
  ],
  invalid: [
    // The near-miss shapes: the hook/op consumed outside the doors…
    { filename: "Foo.viewModel.ts", code: `import { useRefresh } from "@/client.gen/sample";`, errors: [{ messageId: "offdoor" }] },
    { filename: "src/app/_layout.tsx", code: `import { refresh } from "@/client.gen/sample";`, errors: [{ messageId: "offdoor" }] },
    { filename: "Foo.viewModel.ts", code: `import { refreshAccessToken } from "@/lib/skies-client";`, errors: [{ messageId: "offdoor" }] },
    // …and the hand-rolled rotation.
    { filename: "Foo.viewModel.ts", code: `instance.post("/account/refresh", {});`, errors: [{ messageId: "raw" }] },
    { filename: "src/lib/api-helpers.ts", code: `axios.post("/auth/refresh-token");`, errors: [{ messageId: "raw" }] },
  ],
});

// SKYFE030 — no `as never`/`as any`/`as unknown` on a navigation target: the cast silences typed routes, and a
// silenced router lets a drifted route literal compile clean and 404 in prod (the pilot's server-minted routes).
ruleTester.run("no-cast-navigation", plugin.rules["no-cast-navigation"], {
  valid: [
    // Typed route literal — the correct shape (typed routes check it at compile time).
    { filename: "Foo.view.tsx", code: `router.push("/host/settings");` },
    // The typed dynamic shape: a { pathname, params } object, no cast.
    { filename: "Foo.view.tsx", code: `router.push({ pathname: "/host/[id]", params: { id } });` },
    // A cast outside navigation is other rules' business (or none).
    { filename: "Foo.view.tsx", code: `const x = parse(data as any); router.push("/home");` },
    // A non-silencing cast (a concrete type) is not the typed-routes mute button.
    { filename: "Foo.view.tsx", code: `router.push(target as Href);` },
    // Declarative redirect with a literal — fine.
    { filename: "src/app/index.tsx", code: `const F = () => <Redirect href="/login" />;` },
    // Tests may cast freely.
    { filename: "Foo.test.tsx", code: `router.push("/x" as never);` },
  ],
  invalid: [
    // The pilot's exact shape: the cast that muted typed routes while a server-minted route drifted.
    { filename: "Foo.view.tsx", code: `router.push("/host/properties/new" as never);`, errors: [{ messageId: "castNav" }] },
    { filename: "Foo.view.tsx", code: `router.replace(target as any);`, errors: [{ messageId: "castNav" }] },
    // The double cast — the inner `as unknown` is the silencer.
    { filename: "Foo.view.tsx", code: `router.navigate(target as unknown as Href);`, errors: [{ messageId: "castNav" }] },
    // The cast buried inside the typed object shape.
    { filename: "Foo.view.tsx", code: `router.push({ pathname: p as never, params: { id } });`, errors: [{ messageId: "castNav" }] },
    // TanStack: a useNavigate() binding with a cast argument.
    { filename: "src/app/x.tsx", code: `const navigate = useNavigate(); navigate({ to: "/x" } as never);`, errors: [{ messageId: "castNav" }] },
    // The declarative half: href/to on Redirect/Navigate/Link.
    { filename: "Foo.view.tsx", code: `const F = () => <Redirect href={target as never} />;`, errors: [{ messageId: "castHref" }] },
    { filename: "Foo.view.tsx", code: `const F = () => <Link href={route as any}>x</Link>;`, errors: [{ messageId: "castHref" }] },
    { filename: "src/app/x.tsx", code: `const F = () => <Navigate to={next as never} />;`, errors: [{ messageId: "castHref" }] },
  ],
});

// SKYFE031 — handleSubmit(onValid) without the invalid path is a silent validation failure (the mute Save button:
// the failure happens BEFORE the mutation, so SKYFE013/027 never see it). submitOrReveal or a second arg passes.
ruleTester.run("submit-handles-invalid", plugin.rules["submit-handles-invalid"], {
  valid: [
    // Both paths passed by hand.
    { filename: "Foo.viewModel.ts", code: `const submit = form.handleSubmit(onValid, onInvalid);` },
    { filename: "Foo.viewModel.ts", code: `const submit = handleSubmit(save, reveal);` },
    // The blessed shape: the spine's primitive takes handleSubmit by REFERENCE (never a one-arg call).
    { filename: "Foo.viewModel.ts", code: `const submit = submitOrReveal(form.handleSubmit, save, { onInvalid: (f) => form.setFocus(f) });` },
    // out of scope: only the ViewModel owns the form logic.
    { filename: "Foo.view.tsx", code: `const submit = form.handleSubmit(onValid);` },
    // an unrelated one-arg call is not the RHF submit.
    { filename: "Foo.viewModel.ts", code: `const x = handle(onValid);` },
  ],
  invalid: [
    // The pilot's exact bug: one argument — a hidden tab's validation failure left Save completely mute.
    { filename: "Foo.viewModel.ts", code: `const submit = form.handleSubmit((values) => mutation.mutate(values));`, errors: [{ messageId: "silent" }] },
    { filename: "Foo.viewModel.ts", code: `const submit = handleSubmit(save);`, errors: [{ messageId: "silent" }] },
  ],
});

// SKYFE032 — a <Controller> render that never reads fieldState leaves a validated error with no surface on its
// field (the pilot's Description input). Destructured or accessed both count; only inline functions are analyzed.
ruleTester.run("controller-field-state", plugin.rules["controller-field-state"], {
  valid: [
    // fieldState destructured and surfaced — the blessed shape.
    {
      filename: "panels/GeneralPanel.view.tsx",
      code: `const P = () => <Controller control={control} name="description" render={({ field, fieldState }) => <Input value={field.value} error={fieldState.error?.message} />} />;`,
    },
    // accessed without destructuring still counts as reading.
    {
      filename: "panels/GeneralPanel.view.tsx",
      code: `const P = () => <Controller control={control} name="description" render={(props) => <Input value={props.field.value} error={props.fieldState.error?.message} />} />;`,
    },
    // a referenced render component is visible in review — not analyzed.
    { filename: "panels/GeneralPanel.view.tsx", code: `const P = () => <Controller control={control} name="x" render={renderDescription} />;` },
    // a non-Controller render prop is out of scope.
    { filename: "Foo.view.tsx", code: `const P = () => <List render={({ item }) => <Row item={item} />} />;` },
    // tests compose freely.
    { filename: "Foo.test.tsx", code: `render(<Controller name="x" render={({ field }) => <Input {...field} />} />);` },
  ],
  invalid: [
    // The pilot's exact bug: only { field } destructured — the field's validation error had no surface at all.
    {
      filename: "panels/GeneralPanel.view.tsx",
      code: `const P = () => <Controller control={control} name="description" render={({ field }) => <Input value={field.value} onChangeText={field.onChange} />} />;`,
      errors: [{ messageId: "blind" }],
    },
    // a props param that never touches fieldState is the same blindness.
    {
      filename: "Foo.view.tsx",
      code: `const P = () => <Controller control={control} name="amount" render={(props) => <Input {...props.field} />} />;`,
      errors: [{ messageId: "blind" }],
    },
  ],
});

// SKYFE036 — tests live in a spec. A runner's test call outside `.specs/` is flagged once per file; a spec's e2e/
// folder, a local function that only shares the name, and a runner's other exports are not.
ruleTester.run("tests-live-in-specs", plugin.rules["tests-live-in-specs"], {
  valid: [
    // A spec's cases, whatever the runner.
    { filename: "/repo/.specs/0006-deposit-screen/e2e/Deposit.test.tsx", code: `import { it, describe } from "vitest"; describe("x", () => { it("FM-1: y", () => {}); });` },
    { filename: "/repo/.specs/0009-checkout/e2e/checkout.spec.ts", code: `import { test } from "@playwright/test"; test.describe("x", () => { test("FM-1: y", async () => {}); });` },
    { filename: "C:\\repo\\.specs\\0001-a\\e2e\\a.test.ts", code: `test("FM-1: y", () => {});` },
    // A local function named like a runner's is not a test.
    { filename: "src/lib/validate.ts", code: `function test(value) { return value > 0; } export const ok = test(1);` },
    { filename: "src/lib/iter.ts", code: `import { it } from "./iterators"; it(1);` },
    // Other imports from a runner module are not test calls.
    { filename: "src/lib/helpers.ts", code: `import { vi } from "vitest"; vi.fn();` },
  ],
  invalid: [
    // A co-located vitest file: one report for the file, not one per case.
    { filename: "src/deposit/Deposit.test.tsx", code: `import { describe, it } from "vitest"; describe("Deposit", () => { it("works", () => {}); it("again", () => {}); });`, errors: [{ messageId: "outsideSpec" }] },
    // Playwright, including test.describe.
    { filename: "e2e/login.spec.ts", code: `import { test, expect } from "@playwright/test"; test.describe("login", () => {});`, errors: [{ messageId: "outsideSpec" }] },
    // jest globals and bun, including a renamed import.
    { filename: "src/a.test.ts", code: `import { test as check } from "@jest/globals"; check("x", () => {});`, errors: [{ messageId: "outsideSpec" }] },
    { filename: "src/b.test.ts", code: `import { describe } from "bun:test"; describe.skip("x", () => {});`, errors: [{ messageId: "outsideSpec" }] },
    // Globals mode: nothing imported, the runner provides it.
    { filename: "src/c.test.ts", code: `it.each([1, 2])("case %i", (n) => {});`, errors: [{ messageId: "outsideSpec" }] },
    // A spec-like name outside .specs/ is still outside.
    { filename: "src/specs/e2e/d.test.ts", code: `test("FM-1: y", () => {});`, errors: [{ messageId: "outsideSpec" }] },
  ],
});

// eslint-disable-next-line no-console
console.log("@skiesjs/eslint-plugin: all SKYFE rule tests passed");
