"use strict";

const { isTest } = require("../lib/shared.cjs");

// SKYFE027 — a QueryClient carries the app's mutation defaults. The write-side of the state discipline: a bare
// `new QueryClient()` leaves every mutation to hand-roll its own cache invalidation and its own error surface —
// and the screen that forgets ships the pilot bug ("created a category, it only appeared after F5, with no
// toast"; 13 of 43 ViewModels had no invalidation at all). The convention pins ONE construction shape:
// `mutationCache: new MutationCache({ onSuccess, onError })` — success marks every query stale (active ones
// refetch immediately; the safe, slightly-wasteful default that is always correct) and posts the success note;
// failure routes through the feedback seam (the global half of SKYFE013). Scaffolded by the Skies CLI
// as lib/query.ts. Tests and the shared test harness (a test/ or test-utils/ path) construct bare clients freely
// — isolation is their job, defaults are the app's.
module.exports = {
  meta: {
    type: "problem",
    docs: {
      description:
        "A QueryClient is constructed with the mutation defaults wired — mutationCache: new MutationCache({ onSuccess: <invalidate + success note>, onError: <feedback> }) — so every mutation invalidates stale reads and surfaces its outcome by default.",
    },
    messages: {
      missing:
        "SKYFE027: this QueryClient carries no mutation defaults — every mutation is left to hand-roll invalidation and error feedback, and the screen that forgets ships stale lists (the F5-to-see-your-write bug) and silent failures. Construct it with `mutationCache: new MutationCache({ onSuccess: <invalidateQueries + success note>, onError: <feedback seam> })` — the Skies CLI scaffolds it as lib/query.ts.",
      incomplete:
        "SKYFE027: the MutationCache defaults are missing `{{missing}}` — `onSuccess` invalidates every active query (no list is one F5 behind its server) and `onError` routes the failure through the feedback seam (no silent failure). Wire both.",
    },
  },
  create(context) {
    const f = context.filename.replace(/\\/g, "/");
    // The shared test harness lives outside *.test.* (e.g. src/test/providers.tsx) but builds throwaway
    // clients for isolation — the defaults are deliberately absent there.
    if (isTest(f) || /(^|\/)(test|tests|test-utils|testing|__tests__)(\/|\.)/.test(f)) return {};
    const prop = (obj, name) =>
      obj.properties.find(
        (p) =>
          p.type === "Property" &&
          !p.computed &&
          ((p.key.type === "Identifier" && p.key.name === name) || (p.key.type === "Literal" && p.key.value === name)),
      );
    const hasSpread = (obj) => obj.properties.some((p) => p.type === "SpreadElement");
    // `const cache = new MutationCache({...})` declared before the QueryClient — the indirection is still
    // checkable; anything built further away (imported, computed) is trusted as visible-in-review.
    const caches = new Map();
    const checkCacheOptions = (reportNode, optsNode) => {
      if (optsNode && optsNode.type !== "ObjectExpression") return; // built elsewhere — review's job
      const obj = optsNode ?? { properties: [] };
      if (hasSpread(obj)) return; // a spread may carry the handlers
      const missing = ["onSuccess", "onError"].filter((k) => !prop(obj, k));
      if (missing.length)
        context.report({ node: reportNode, messageId: "incomplete", data: { missing: missing.join("` and `") } });
    };
    return {
      VariableDeclarator(node) {
        if (
          node.id.type === "Identifier" &&
          node.init &&
          node.init.type === "NewExpression" &&
          node.init.callee.type === "Identifier" &&
          node.init.callee.name === "MutationCache"
        )
          caches.set(node.id.name, node.init.arguments[0] ?? null);
      },
      NewExpression(node) {
        if (node.callee.type !== "Identifier" || node.callee.name !== "QueryClient") return;
        const arg = node.arguments[0];
        if (!arg) return context.report({ node, messageId: "missing" });
        if (arg.type !== "ObjectExpression") return; // an options factory — trusted, visible in review
        const cacheProp = prop(arg, "mutationCache");
        if (!cacheProp) {
          if (!hasSpread(arg)) context.report({ node, messageId: "missing" });
          return;
        }
        const v = cacheProp.value;
        if (v.type === "NewExpression" && v.callee.type === "Identifier" && v.callee.name === "MutationCache")
          return checkCacheOptions(node, v.arguments[0] ?? null);
        if (v.type === "Identifier" && caches.has(v.name)) return checkCacheOptions(node, caches.get(v.name));
        // an imported/composed cache — it exists; its wiring is review's job, not a false positive's.
      },
    };
  },
};
