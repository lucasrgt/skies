"use strict";

// Shared vocabulary for the SKYFE rules: file-role predicates (View, ViewModel, route, seams), the import helper,
// and the small AST walkers the routing rules use. Pure functions, no I/O.

const GENERATED_CLIENT = /(^|\/)client\.gen(\/|$)/;     // every generated file, including contract models
const GENERATED_OPERATIONS = /(^|\/)client\.gen(?:\/(?!model(?:\/|$))|$)/; // transport/hooks, never model values
const DATA_LIBS = /^(axios)$|^@tanstack\/react-query$/;  // raw transport / the Model layer
const MOCKS = /(^|\/)(__mocks__|fixtures)(\/|$)|^msw($|\/)/;

const isView = (f) => /\.view\.tsx?$/.test(f);
const isViewModel = (f) => /\.viewModel\.ts$/.test(f);
const isTest = (f) => /\.(test|spec)\.[jt]sx?$/.test(f);
const isGenerated = (f) => GENERATED_CLIENT.test(f.replace(/\\/g, "/"));
// The data doors are two: a screen's *.viewModel.ts (per-screen), AND the framework's auth/routing
// infrastructure — session bootstrap + route guards. Both legitimately read the generated client (refresh, me,
// lifecycle); screens still may NOT bypass their ViewModel. Scoped to lib/session + lib/guards so it stays a
// principled allowance, not a general escape hatch. (This is the cross-cutting infra Angular puts behind
// CanActivate / an AuthService — framework primitives the app composes, not a DSL.)
const isInfraDataDoor = (f) => /(^|\/)lib\/(session|guards)(\.|\/)/.test(f.replace(/\\/g, "/"));

// A type-only import (`import type { X }`) is erased at runtime — it is the shared contract vocabulary, not
// data access. A View taking `kind: LegalDocKind` is wired correctly; only a *value* import (a hook, the
// client) is a data path. So the data-door rules exempt type imports; the contract types are everyone's.
const isTypeOnly = (node) =>
  node.importKind === "type" ||
  (node.specifiers.length > 0 && node.specifiers.every((s) => s.importKind === "type"));

/** Report on any *value* import whose source matches `pattern`. */
function forbidImport(context, pattern, messageId) {
  return {
    ImportDeclaration(node) {
      if (!isTypeOnly(node) && pattern.test(node.source.value)) {
        context.report({ node, messageId });
      }
    },
  };
}

// ── Routing vocabulary (recognized for TanStack Router and React Router) ───────────────────────────────────────
// The routing rules police a SHAPE (declarative redirect, guarded back, param presence), not a router runtime —
// so they recognize each router's idiom but depend on neither. "Ship the standard, not the adapter."

// A route file — the navigation layer, the only layer that may redirect or read route params. Three layouts name it:
// `app/` (TanStack Start, React Router's framework mode), TanStack Router's file-based `src/routes/**`, and React
// Router's `routes/` module convention. A segment must be exactly `app` or `routes` (`_app/` is a layout route
// inside `routes/`, still a route by its parent). The generated route tree (`routeTree.gen.ts`) and tests are not.
const isRoute = (f) => {
  const p = f.replace(/\\/g, "/");
  return /(^|\/)(app|routes)\//.test(p) && !/\.gen\.[jt]sx?$/.test(p) && !isTest(p);
};
// The nav seam — the single guarded back handler (safeBack / useGoBack). The one place a bare back() is allowed.
const isNavSeam = (f) => /(^|\/)lib\/(nav|useGoBack)(\.|\/)/.test(f.replace(/\\/g, "/"));
// The "am I signed in?" boolean a guard must NOT branch a redirect on: it collapses the tri-state (loading vs
// anonymous) into one bit, so the redirect fires before the session settles. Branch on a SessionState instead.
const AUTH_BOOL = /^(is)?(authenticated|authed|loggedin|signedin)$/i;
// The words of an identifier or storage key (camelCase, snake_case, kebab, dotted), lowercased: `accessToken` →
// [access, token], `auth_token` → [auth, token], `authorName` → [author, name]. The session rules match whole words so
// "author" never reads as "auth"; the Flutter twins split the same way.
const words = (text) =>
  String(text)
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .split(/[^A-Za-z0-9]+/)
    .filter(Boolean)
    .map((w) => w.toLowerCase());
// A key that names a session credential by one of its words (SKYFE016 / SKYFL016).
const SESSION_WORDS = new Set(["token", "tokens", "jwt", "session", "auth"]);
const isSessionKey = (key) => words(key).some((w) => SESSION_WORDS.has(w));
// A route param whose ABSENCE yields a ghost screen — an id the View needs. Optional filter/search params don't
// qualify; only id-shaped names render an empty detail when missing.
const ID_PARAM = /(^id$)|Id$/;

/** Whether `node` sits lexically inside a useEffect / useLayoutEffect callback (where a redirect re-fires every render). */
function inEffect(node) {
  for (let p = node.parent; p; p = p.parent) {
    if (
      p.type === "CallExpression" &&
      p.callee.type === "Identifier" &&
      (p.callee.name === "useEffect" || p.callee.name === "useLayoutEffect")
    )
      return true;
  }
  return false;
}

/** The nearest enclosing function of `node` (the component body), or null. */
function enclosingFunction(node) {
  for (let p = node.parent; p; p = p.parent) {
    if (
      p.type === "FunctionDeclaration" ||
      p.type === "FunctionExpression" ||
      p.type === "ArrowFunctionExpression"
    )
      return p;
  }
  return null;
}

/** The leaf name of `x` / `x.y` when it reads as an auth boolean (AUTH_BOOL), else null. */
function authBoolName(expr) {
  if (expr.type === "Identifier" && AUTH_BOOL.test(expr.name)) return expr.name;
  if (
    expr.type === "MemberExpression" &&
    expr.property.type === "Identifier" &&
    AUTH_BOOL.test(expr.property.name)
  )
    return expr.property.name;
  return null;
}

/** Whether a JSX element is a declarative redirect — `<Navigate>` (TanStack Router and React Router alike). */
function isRedirectElement(arg) {
  if (!arg || arg.type !== "JSXElement") return false;
  const name = arg.openingElement.name;
  return name.type === "JSXIdentifier" && name.name === "Navigate";
}

/** Whether an expression is a router's redirect call: TanStack's `redirect({ to })`, React Router's `redirect("/x")`. */
function isRedirectCall(arg) {
  return !!arg && arg.type === "CallExpression" && arg.callee.type === "Identifier" && arg.callee.name === "redirect";
}

/**
 * Whether a statement (or block) redirects: returns `<Navigate …/>`, or returns/throws a `redirect(…)` (a TanStack
 * `beforeLoad`, a React Router loader).
 */
function returnsRedirect(stmt) {
  const body = stmt.type === "BlockStatement" ? stmt.body : [stmt];
  return body.some(
    (s) =>
      (s.type === "ReturnStatement" && (isRedirectElement(s.argument) || isRedirectCall(s.argument))) ||
      (s.type === "ThrowStatement" && isRedirectCall(s.argument)),
  );
}

/** Walk every node under `root` (skipping `parent` back-edges), calling `fn`; `fn` returning true stops the walk. */
function walk(root, fn) {
  let stop = false;
  const visit = (node) => {
    if (stop || !node || typeof node.type !== "string") return;
    if (fn(node) === true) {
      stop = true;
      return;
    }
    for (const key of Object.keys(node)) {
      if (key === "parent") continue;
      const v = node[key];
      if (Array.isArray(v)) v.forEach(visit);
      else if (v && typeof v.type === "string") visit(v);
    }
  };
  visit(root);
}

/** Whether the identifier `name` appears anywhere in `node`'s subtree (a value reference). */
function identifierAppears(node, name) {
  let found = false;
  walk(node, (n) => (n.type === "Identifier" && n.name === name ? (found = true) : false));
  return found;
}

/**
 * The names that stand in for a param in a presence guard: the param itself plus any local initialized from it —
 * a coalesce (`const x = a ?? param`) or a rename (`const x = param`). Guarding any of them guards the param, so a
 * `const id = a ?? b; if (!id) return <Navigate/>` is recognized, not falsely flagged.
 */
function aliasesOf(fn, base) {
  const names = new Set([base]);
  walk(fn.body, (n) => {
    if (
      n.type === "VariableDeclarator" &&
      n.id.type === "Identifier" &&
      n.init &&
      !names.has(n.id.name) &&
      [...names].some((nm) => identifierAppears(n.init, nm))
    )
      names.add(n.id.name);
    return false;
  });
  return names;
}

/**
 * Whether an `if` test guarantees a redirect when some name in `names` is absent: a bare `!X`, a `||`-chain
 * with `!X` as a disjunct (`!a || !b` redirects when either is missing), or the spine's union check
 * (`X.status === "missing"` off `requiredParam(...)`). `&&` is rejected — `!X && y` does NOT redirect on `X`
 * alone, so it is not a sound presence guard.
 */
function testGuardsAny(test, names) {
  if (test.type === "UnaryExpression" && test.operator === "!" && test.argument.type === "Identifier")
    return names.has(test.argument.name);
  if (test.type === "LogicalExpression" && test.operator === "||")
    return testGuardsAny(test.left, names) || testGuardsAny(test.right, names);
  // The spine's shape: `const id = requiredParam(param); if (id.status === "missing") return <Navigate/>`.
  if (
    test.type === "BinaryExpression" &&
    test.operator === "===" &&
    test.left.type === "MemberExpression" &&
    !test.left.computed &&
    test.left.object.type === "Identifier" &&
    names.has(test.left.object.name) &&
    test.left.property.type === "Identifier" &&
    test.left.property.name === "status" &&
    test.right.type === "Literal" &&
    test.right.value === "missing"
  )
    return true;
  return false;
}

/** Whether a statement (or block) throws: TanStack's `throw notFound()` / `throw redirect(…)`, or an error boundary. */
function throwsOut(stmt) {
  const body = stmt.type === "BlockStatement" ? stmt.body : [stmt];
  return body.some((s) => s.type === "ThrowStatement");
}

// An assertion call that throws on a falsy first argument: `invariant(id, "…")` (tiny-invariant), `assert(id)`.
const ASSERTION = /^(invariant|assert)$/;

/**
 * Whether `fn`'s body guards the absence of any name in `names`: `if (<guards X>) return <Navigate …/>`, the same test
 * followed by a `throw` (TanStack's `throw notFound()`, an error boundary), or an `invariant(X)` / `assert(X)` call.
 * Each keeps a param-less hit off the ghost screen; a `return null` does not (it is the blank ghost itself).
 */
function hasPresenceGuard(fn, names) {
  let found = false;
  walk(fn.body, (node) => {
    if (
      node.type === "IfStatement" &&
      testGuardsAny(node.test, names) &&
      (returnsRedirect(node.consequent) || throwsOut(node.consequent))
    )
      return (found = true);
    if (
      node.type === "CallExpression" &&
      node.callee.type === "Identifier" &&
      ASSERTION.test(node.callee.name) &&
      node.arguments[0]?.type === "Identifier" &&
      names.has(node.arguments[0].name)
    )
      return (found = true);
    return false;
  });
  return found;
}

module.exports = {
  GENERATED_CLIENT,
  GENERATED_OPERATIONS,
  DATA_LIBS,
  MOCKS,
  isView,
  isViewModel,
  isTest,
  isGenerated,
  isInfraDataDoor,
  isTypeOnly,
  forbidImport,
  isRoute,
  isNavSeam,
  AUTH_BOOL,
  ID_PARAM,
  words,
  isSessionKey,
  inEffect,
  enclosingFunction,
  authBoolName,
  isRedirectElement,
  returnsRedirect,
  walk,
  identifierAppears,
  aliasesOf,
  testGuardsAny,
  hasPresenceGuard,
};
