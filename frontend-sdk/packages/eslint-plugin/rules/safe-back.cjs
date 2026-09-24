"use strict";

const { isView, isRoute, isNavSeam } = require("../lib/shared.cjs");

// SKYFE019 — no bare `router.back()` / `history.back()`. On web a deep-linked / refreshed screen has no in-app
// history, so back() is a no-op and the "Back" button is dead (the pilot migrated ~13 screens off it). Route every
// Back affordance through a guarded helper — the spine's `safeBack(router, fallback)` / an app `useGoBack` — that
// pops when it can and otherwise replaces to a parent. A file that already guards with `canGoBack` is fine; the
// nav seam (where the helper lives) is exempt. Scoped to the screens/routes that hold Back buttons.
module.exports = {
  meta: {
    type: "problem",
    docs: {
      description:
        "No bare router.back() — on web a deep-linked screen has no in-app history, so it is a no-op (a dead Back button). Use a guarded helper (safeBack / useGoBack) that falls back to a parent.",
    },
    messages: {
      bareBack:
        "SKYFE019: no bare `{{call}}` — on web a deep-linked / refreshed screen has no in-app history, so it does nothing (a dead 'Back' button). Use a guarded helper: `useGoBack(fallback)` / `safeBack(router, fallback)` (pops when it can, else replaces to a parent).",
    },
  },
  create(context) {
    const f = context.filename.replace(/\\/g, "/");
    if ((!isView(f) && !isRoute(f)) || isNavSeam(f)) return {};
    // An inline `canGoBack`-guarded back() is the safe shape — exempt files that already do it.
    if (/canGoBack/.test((context.sourceCode ?? context.getSourceCode()).getText())) return {};
    return {
      CallExpression(node) {
        const callee = node.callee;
        if (callee.type !== "MemberExpression" || callee.computed) return;
        if (callee.property.type !== "Identifier" || callee.property.name !== "back") return;
        const obj = callee.object;
        const isRouterBack = obj.type === "Identifier" && obj.name === "router";
        const isHistoryBack =
          obj.type === "MemberExpression" && obj.property.type === "Identifier" && obj.property.name === "history";
        if (!isRouterBack && !isHistoryBack) return;
        context.report({ node, messageId: "bareBack", data: { call: isRouterBack ? "router.back()" : "history.back()" } });
      },
    };
  },
};
