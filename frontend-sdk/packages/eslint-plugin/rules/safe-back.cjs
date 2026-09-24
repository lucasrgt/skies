"use strict";

const { isView, isRoute, isNavSeam } = require("../lib/shared.cjs");

// SKYFE019 — no bare `history.back()` / `navigate(-1)`. A deep-linked / refreshed screen has no in-app history, so
// back() is a no-op (or leaves the app) and the "Back" button is dead (the pilot migrated ~13 screens off it). Route
// every Back affordance through a guarded helper — the spine's `safeBack(router, fallback)` / an app `useGoBack` —
// that pops when it can and otherwise replaces to a parent. Recognizes `history.back()` on any receiver
// (`window.history`, TanStack's `router.history`) and React Router's `navigate(-1)` off a `useNavigate()` binding. A
// file that already guards with `canGoBack` is fine; the nav seam (where the helper lives) is exempt. Scoped to the
// screens/routes that hold Back buttons.
module.exports = {
  meta: {
    type: "problem",
    docs: {
      description:
        "No bare history.back() / navigate(-1) — a deep-linked screen has no in-app history, so it is a no-op (a dead Back button). Use a guarded helper (safeBack / useGoBack) that falls back to a parent.",
    },
    messages: {
      bareBack:
        "SKYFE019: no bare `{{call}}` — a deep-linked / refreshed screen has no in-app history, so it does nothing (a dead 'Back' button). Use a guarded helper: `useGoBack(fallback)` / `safeBack(router, fallback)` (pops when it can, else replaces to a parent).",
    },
  },
  create(context) {
    const f = context.filename.replace(/\\/g, "/");
    if ((!isView(f) && !isRoute(f)) || isNavSeam(f)) return {};
    // An inline `canGoBack`-guarded back() is the safe shape — exempt files that already do it.
    if (/canGoBack/.test((context.sourceCode ?? context.getSourceCode()).getText())) return {};
    // Identifiers bound from `useNavigate()` — so React Router's `navigate(-1)` is recognized.
    const navigators = new Set();
    return {
      VariableDeclarator(node) {
        if (
          node.id.type === "Identifier" &&
          node.init &&
          node.init.type === "CallExpression" &&
          node.init.callee.type === "Identifier" &&
          node.init.callee.name === "useNavigate"
        )
          navigators.add(node.id.name);
      },
      CallExpression(node) {
        const callee = node.callee;
        if (callee.type === "Identifier" && navigators.has(callee.name)) {
          const arg = node.arguments[0];
          const isMinusOne =
            arg &&
            arg.type === "UnaryExpression" &&
            arg.operator === "-" &&
            arg.argument.type === "Literal" &&
            arg.argument.value === 1;
          if (isMinusOne) context.report({ node, messageId: "bareBack", data: { call: `${callee.name}(-1)` } });
          return;
        }
        if (callee.type !== "MemberExpression" || callee.computed) return;
        if (callee.property.type !== "Identifier" || callee.property.name !== "back") return;
        const obj = callee.object;
        const isHistory =
          (obj.type === "Identifier" && obj.name === "history") ||
          (obj.type === "MemberExpression" && obj.property.type === "Identifier" && obj.property.name === "history");
        if (isHistory) context.report({ node, messageId: "bareBack", data: { call: "history.back()" } });
      },
    };
  },
};
