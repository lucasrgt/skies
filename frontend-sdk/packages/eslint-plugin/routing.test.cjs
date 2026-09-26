"use strict";

// The routing, session, and security SKYFE rules (SKYFE015-022, 029, 030), pinned with RuleTester on both edges like
// the rest in index.test.cjs, which requires this file. They recognize TanStack Router and React Router idioms.

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

// SKYFE015 — no imperative redirect inside useEffect (a guard redirect must be a declarative <Navigate>, not an effect
// that flashes or loops). Recognizes a router instance and a useNavigate() binding; user-action navigation stays allowed.
ruleTester.run("no-router-replace-in-effect", plugin.rules["no-router-replace-in-effect"], {
  valid: [
    // Declarative redirect — the correct shape.
    { filename: "Foo.view.tsx", code: `export const F = () => (done ? <Navigate to="/home" /> : null);` },
    { filename: "src/app/x.tsx", code: `export const F = () => (done ? <Navigate to="/home" replace /> : null);` },
    // Imperative navigation on a user action (not in an effect) — a navigation completion, allowed.
    { filename: "Foo.view.tsx", code: `export const F = () => { const onClick = () => router.navigate({ to: "/home" }); return null; };` },
    { filename: "src/app/x.tsx", code: `export const F = () => { const navigate = useNavigate(); const onClick = () => navigate({ to: "/home" }); return null; };` },
    // A history read inside an effect is not a navigation.
    { filename: "Foo.view.tsx", code: `export const F = () => { useEffect(() => { track(router.state.location); }, []); };` },
    // out of scope: not a View or route.
    { filename: "Foo.viewModel.ts", code: `useEffect(() => { router.navigate({ to: "/home" }); }, []);` },
  ],
  invalid: [
    // A router instance (TanStack's useRouter(), React Router's data router) navigating in an effect.
    { filename: "Foo.view.tsx", code: `export const F = () => { useEffect(() => { router.navigate({ to: "/home" }); }, []); };`, errors: [{ messageId: "effectReplace" }] },
    { filename: "Foo.view.tsx", code: `export const F = () => { useLayoutEffect(() => { if (x) router.navigate("/x"); }, [x]); };`, errors: [{ messageId: "effectReplace" }] },
    // React Router: a useNavigate() binding called with a path in an effect.
    { filename: "src/app/x.tsx", code: `export const F = () => { const navigate = useNavigate(); useEffect(() => { navigate("/home", { replace: true }); }, []); };`, errors: [{ messageId: "effectReplace" }] },
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
    { filename: "src/app/routes/index.tsx", code: `function H() { if (!chatId) return <Navigate to="/m" />; return null; }` },
    // out of scope: a plain feature view is not a route/guard.
    { filename: "Foo.view.tsx", code: `function H() { if (!isAuthenticated) return <Navigate to="/login" />; return null; }` },
  ],
  invalid: [
    { filename: "src/app/routes/index.tsx", code: `function H() { if (!session.isAuthenticated) return <Navigate to="/login" />; return null; }`, errors: [{ messageId: "boolRedirect" }] },
    { filename: "src/lib/guards/Admin.tsx", code: `function H() { if (!isAuthenticated) return <Navigate to="/login" />; return null; }`, errors: [{ messageId: "boolRedirect" }] },
  ],
});

// SKYFE018 — a route reading a required id param must guard its absence with a declarative redirect.
ruleTester.run("route-param-guard", plugin.rules["route-param-guard"], {
  valid: [
    // The param is guarded before the View renders.
    { filename: "src/app/messaging/chat.tsx", code: `function R() { const { chatId } = useParams(); if (!chatId) return <Navigate to="/messaging" />; return <Chat chatId={chatId} />; }` },
    // Guarding a COALESCED value covers every param feeding it — `const id = a ?? b; if (!id) …` is not flagged.
    { filename: "src/app/x.tsx", code: `function R() { const { propertyId, id } = useParams(); const resolved = propertyId ?? id; if (!resolved) return <Navigate to="/" />; return <V propertyId={resolved} />; }` },
    // Guarding a renamed param covers it.
    { filename: "src/app/y.tsx", code: `function R() { const { id } = useParams(); const propertyId = id; if (!propertyId) return <Navigate to="/" />; return <V propertyId={propertyId} />; }` },
    // Two required params guarded together by a `||`-chain — both covered (redirects if either is missing).
    { filename: "src/app/z.tsx", code: `function R() { const { id, propertyId } = useParams(); if (!id || !propertyId) return <Navigate to="/" />; return <V id={id} propertyId={propertyId} />; }` },
    // The spine's union shape: requiredParam() + a status === "missing" branch guards the param.
    { filename: "src/app/chat.tsx", code: `function R() { const { chatId } = useParams(); const id = requiredParam(chatId); if (id.status === "missing") return <Navigate to="/messaging" />; return <Chat chatId={id.value} />; }` },
    // A strict TanStack read is guaranteed by the matched route.
    { filename: "src/app/chat.tsx", code: `function R() { const { chatId } = useParams({ from: "/messaging/$chatId" }); return <Chat chatId={chatId} />; }` },
    // A non-id param (an optional filter) does not ghost — not required.
    { filename: "src/app/list.tsx", code: `function R() { const { tab } = useParams(); return <List tab={tab} />; }` },
    // out of scope: not a route file.
    { filename: "Foo.view.tsx", code: `function R() { const { chatId } = useParams(); return <Chat chatId={chatId} />; }` },
  ],
  invalid: [
    // React Router: a bare useParams() types every param as possibly undefined.
    { filename: "src/app/messaging/chat.tsx", code: `function R() { const { chatId } = useParams(); return <Chat chatId={chatId} />; }`, errors: [{ messageId: "unguarded" }] },
    { filename: "src/app/property/detail.tsx", code: `function R() { const { id } = useParams(); return <Detail id={id} />; }`, errors: [{ messageId: "unguarded" }] },
    // TanStack: a non-strict read opts out of the route's guarantee.
    { filename: "src/app/property/detail.tsx", code: `function R() { const { id } = useParams({ strict: false }); return <Detail id={id} />; }`, errors: [{ messageId: "unguarded" }] },
  ],
});

// SKYFE019 — no bare history.back() / navigate(-1); route Back through a guarded helper.
ruleTester.run("safe-back", plugin.rules["safe-back"], {
  valid: [
    // The guarded helper — the correct shape.
    { filename: "Header.view.tsx", code: `const onBack = useGoBack("/");` },
    // An inline canGoBack-guarded back() is fine (the file references canGoBack).
    { filename: "Header.view.tsx", code: `const onBack = () => { if (router.history.canGoBack()) router.history.back(); else router.navigate({ to: "/" }); };` },
    // The nav seam (where the helper lives) is exempt.
    { filename: "src/lib/useGoBack.ts", code: `export const useGoBack = () => () => window.history.back();` },
    // A forward navigation is not the banned call.
    { filename: "Header.view.tsx", code: `const navigate = useNavigate(); const onNext = () => navigate("/x");` },
  ],
  invalid: [
    { filename: "Header.view.tsx", code: `const onBack = () => history.back();`, errors: [{ messageId: "bareBack" }] },
    { filename: "src/app/notifications.tsx", code: `const onBack = () => window.history.back();`, errors: [{ messageId: "bareBack" }] },
    // TanStack: the router's history.
    { filename: "Header.view.tsx", code: `const onBack = () => router.history.back();`, errors: [{ messageId: "bareBack" }] },
    // React Router: navigate(-1).
    { filename: "Header.view.tsx", code: `const navigate = useNavigate(); const onBack = () => navigate(-1);`, errors: [{ messageId: "bareBack" }] },
  ],
});

// SKYFE020 — the API base URL comes from configuration, not a hardcoded host baked into the client's construction.
ruleTester.run("no-hardcoded-base-url", plugin.rules["no-hardcoded-base-url"], {
  valid: [
    // env-driven with a relative fallback (web) — the blessed shape.
    { filename: "src/lib/skies-client.ts", code: `const c = axios.create({ baseURL: import.meta.env.VITE_API_URL ?? "" });` },
    // env-driven with an env fallback.
    { filename: "src/lib/skies-client.ts", code: `const c = axios.create({ baseURL: import.meta.env.VITE_API_URL ?? "http://localhost:8080" });` },
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
    { filename: "src/app/login.tsx", code: `function R() { const { returnTo } = useSearch(); router.navigate({ to: "/home" }); return null; }` },
    // The allowlisted mapping is the blessed shape: the raw param picks a KNOWN route, never becomes one.
    { filename: "src/app/login.tsx", code: `function R() { const { returnTo } = useSearch(); const to = ROUTES.has(returnTo) ? returnTo : "/home"; return null; }` },
    // out of scope: a viewModel does not navigate.
    { filename: "Foo.viewModel.ts", code: `const { returnTo } = useSearch(); router.navigate({ to: returnTo });` },
  ],
  invalid: [
    // TanStack: useSearch() into router.navigate / a useNavigate() binding.
    { filename: "src/app/login.tsx", code: `function R() { const { returnTo } = useSearch(); router.navigate({ to: returnTo }); return null; }`, errors: [{ messageId: "openRedirect" }] },
    { filename: "src/app/login.tsx", code: `function R() { const search = useSearch(); const navigate = useNavigate(); navigate({ to: search.next }); return null; }`, errors: [{ messageId: "openRedirect" }] },
    // React Router: useSearchParams() into location.href.
    { filename: "src/app/login.tsx", code: `function R() { const [params] = useSearchParams(); window.location.href = params.get("next"); return null; }`, errors: [{ messageId: "openRedirect" }] },
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
    { filename: "Login.viewModel.ts", code: `sessionStorage.setItem("jwt", t);`, errors: [{ messageId: "storage" }] },
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
    { filename: "Foo.view.tsx", code: `router.navigate({ to: "/host/settings" });` },
    // The typed dynamic shape: { to, params }, no cast.
    { filename: "Foo.view.tsx", code: `router.navigate({ to: "/host/$id", params: { id } });` },
    // A cast outside navigation is other rules' business (or none).
    { filename: "Foo.view.tsx", code: `const x = parse(data as any); router.navigate({ to: "/home" });` },
    // A non-silencing cast (a concrete type) is not the typed-routes mute button.
    { filename: "Foo.view.tsx", code: `router.navigate({ to: target as RoutePath });` },
    // Declarative redirect with a literal — fine.
    { filename: "src/app/index.tsx", code: `const F = () => <Navigate to="/login" />;` },
    // Tests may cast freely.
    { filename: "Foo.test.tsx", code: `router.navigate({ to: "/x" as never });` },
  ],
  invalid: [
    // The pilot's exact shape: the cast that muted typed routes while a server-minted route drifted.
    { filename: "Foo.view.tsx", code: `router.navigate({ to: "/host/properties/new" as never });`, errors: [{ messageId: "castNav" }] },
    { filename: "Foo.view.tsx", code: `router.navigate(target as any);`, errors: [{ messageId: "castNav" }] },
    // The double cast — the inner `as unknown` is the silencer.
    { filename: "Foo.view.tsx", code: `router.navigate({ to: target as unknown as RoutePath });`, errors: [{ messageId: "castNav" }] },
    // The cast buried inside the typed object shape.
    { filename: "Foo.view.tsx", code: `router.navigate({ to: "/host/$id", params: { id: p as never } });`, errors: [{ messageId: "castNav" }] },
    // A useNavigate() binding with a cast argument.
    { filename: "src/app/x.tsx", code: `const navigate = useNavigate(); navigate({ to: "/x" } as never);`, errors: [{ messageId: "castNav" }] },
    // The declarative half: `to` on Navigate/Link.
    { filename: "Foo.view.tsx", code: `const F = () => <Link to={route as any}>x</Link>;`, errors: [{ messageId: "castHref" }] },
    { filename: "src/app/x.tsx", code: `const F = () => <Navigate to={next as never} />;`, errors: [{ messageId: "castHref" }] },
  ],
});
