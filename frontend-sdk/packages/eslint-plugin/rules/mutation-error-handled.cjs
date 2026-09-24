"use strict";

const { isViewModel, walk } = require("../lib/shared.cjs");

// SKYFE013 — every mutation surfaces its error. A react-query `.mutate(...)` / `.mutateAsync(...)` whose options
// object has no `onError` is a SILENT failure: the command fails and the user sees nothing. The front-side of the
// backend's error_handling discipline — there, a Result's sad path is forced; here, a mutation must route its
// error somewhere (a toast, a saveError state, a banner). Scoped to *.viewModel.ts (the data door owns commands).
// It enforces PRESENCE of onError, not what it does (anti-test-theater): wiring the error out is the bar, the
// UX of it stays per-screen judgment.
//
// `{ globalSurface: true }` — for apps running the SKYFE027 mutation defaults: the QueryClient's global
// MutationCache.onError already routes EVERY failure through the feedback seam (and react-query fires it
// regardless of per-call handlers), so a bare `.mutate()` is surfaced by construction and the per-call demand
// would be the redundant second handler this rule refuses to require. The empty `onError: () => {}` stays
// flagged either way — it is dead paperwork. Set the option only alongside `query-client-defaults: "error"`
// (SKYFE027 is what makes the claim true).
module.exports = {
  meta: {
    type: "problem",
    docs: {
      description:
        "Every mutation surfaces its failure — via an onError handler, a read .isError state, or a try/catch around mutateAsync (no silent failure). With { globalSurface: true } (the SKYFE027 defaults wired), the global onError is the surface and only an empty onError is flagged.",
    },
    schema: [
      {
        type: "object",
        properties: { globalSurface: { type: "boolean" } },
        additionalProperties: false,
      },
    ],
    messages: {
      unhandled:
        "SKYFE013: a mutation must surface its error (no silent failure; the front-side of the backend's error_handling). Use ANY ONE: pass `onError` to .{{method}}(args, { onError }); OR read `{{name}}.isError` and expose it as state the View renders; OR `await {{name}}.mutateAsync()` inside a try/catch that sets an error surface.",
      empty:
        "SKYFE013: this `onError` swallows the failure — an empty handler is the silent failure with paperwork. Route the error somewhere the user can see (set an error state, show a toast), or read `{{name}}.isError` as state instead.",
    },
  },
  create(context) {
    const f = context.filename.replace(/\\/g, "/");
    if (!isViewModel(f)) return {};
    const globalSurface = context.options[0]?.globalSurface === true;
    // The whole-file text lets us see the `.isError` surface pattern: react-query's idiom is to read the mutation
    // handle's `isError`/`error` and expose it as returned state (the View renders it via <ErrorBanner> / toast),
    // which is a real error surface — just not an inline onError. Recognizing it stops the rule demanding a
    // redundant second handler (which would be the very test-theater the rule exists to prevent).
    const src = (context.sourceCode ?? context.getSourceCode()).getText();
    const hasKey = (obj, name) =>
      obj &&
      obj.type === "ObjectExpression" &&
      obj.properties.some(
        (p) =>
          (p.type === "Property" || p.type === "SpreadElement") &&
          (p.type === "SpreadElement" || // a spread may carry onError — don't false-positive
            (!p.computed &&
              ((p.key.type === "Identifier" && p.key.name === name) ||
                (p.key.type === "Literal" && p.key.value === name)))),
      );
    // mutateAsync REJECTS on failure, so a try/catch around it IS the handler. .mutate() is fire-and-forget (it
    // never throws), so a try/catch around .mutate() catches nothing — only mutateAsync earns this pass. The try
    // must be in the same function as the call (a try in an outer function across a callback boundary doesn't wrap
    // the await), so we stop the walk at the first enclosing function.
    const inTryBlock = (node) => {
      let prev = node;
      for (let n = node.parent; n; prev = n, n = n.parent) {
        if (n.type === "TryStatement" && n.block === prev) return true;
        if (
          n.type === "FunctionDeclaration" ||
          n.type === "FunctionExpression" ||
          n.type === "ArrowFunctionExpression"
        )
          return false;
      }
      return false;
    };
    // mutateAsync(...).catch(...) is the promise-equivalent of the try/catch above — the rejection path is
    // acknowledged. The rule trusts the STRUCTURE (you handled the rejection); whether the handler body is
    // meaningful is the same human judgment as a try/catch body, not something a lint rule should grade.
    const hasCatch = (node) =>
      node.parent &&
      node.parent.type === "MemberExpression" &&
      node.parent.object === node &&
      node.parent.property.type === "Identifier" &&
      node.parent.property.name === "catch" &&
      node.parent.parent &&
      node.parent.parent.type === "CallExpression";
    // A thin wrapper that RETURNS the mutateAsync promise (`(args) => mut.mutateAsync(...)` or `return
    // mut.mutateAsync(...)`) delegates error handling to whoever awaits it — propagation, not a silent swallow.
    // Unwrap `as`-casts / parens that wrap the returned expression.
    const isReturned = (node) => {
      let n = node;
      while (
        n.parent &&
        (n.parent.type === "TSAsExpression" ||
          n.parent.type === "TSNonNullExpression" ||
          n.parent.type === "ParenthesizedExpression")
      )
        n = n.parent;
      const p = n.parent;
      if (!p) return false;
      if (p.type === "ReturnStatement") return true;
      if (p.type === "ArrowFunctionExpression" && p.body === n) return true;
      return false;
    };
    return {
      CallExpression(node) {
        const callee = node.callee;
        if (callee.type !== "MemberExpression" || callee.computed) return;
        if (callee.property.type !== "Identifier") return;
        const method = callee.property.name;
        if (method !== "mutate" && method !== "mutateAsync") return;
        // A) inline onError in the options arg — but an EMPTY handler (`onError: () => {}`) is the silent
        // failure with paperwork: the rule's whole point, defeated by its own escape hatch. Flag it.
        const opts = node.arguments[1];
        if (hasKey(opts, "onError")) {
          const handler = opts.properties.find(
            (p) =>
              p.type === "Property" &&
              !p.computed &&
              ((p.key.type === "Identifier" && p.key.name === "onError") ||
                (p.key.type === "Literal" && p.key.value === "onError")),
          );
          const fn = handler?.value;
          const objName = callee.object.type === "Identifier" ? callee.object.name : "the mutation";
          if (
            fn &&
            (fn.type === "ArrowFunctionExpression" || fn.type === "FunctionExpression") &&
            fn.body.type === "BlockStatement" &&
            fn.body.body.length === 0
          )
            context.report({ node: handler, messageId: "empty", data: { name: objName } });
          return;
        }
        // With the SKYFE027 defaults wired, the global MutationCache.onError surfaces every failure — a bare
        // call is handled by construction, and only the empty handler above remains worth flagging.
        if (globalSurface) return;
        // B) the mutation handle's error state is read in this file (surfaced as state the View renders).
        const obj = callee.object;
        const name = obj.type === "Identifier" ? obj.name : null;
        if (name && new RegExp(`\\b${name}\\.(isError|error|failureReason)\\b`).test(src)) return;
        // C) mutateAsync awaited inside a try/catch, chained with .catch(), or returned (propagated to the caller).
        if (method === "mutateAsync" && (inTryBlock(node) || hasCatch(node) || isReturned(node))) return;
        context.report({ node, messageId: "unhandled", data: { method, name: name ?? "the mutation" } });
      },
    };
  },
};
