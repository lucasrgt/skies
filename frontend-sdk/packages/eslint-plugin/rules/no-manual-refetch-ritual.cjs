"use strict";

const { isViewModel } = require("../lib/shared.cjs");

// SKYFE028 — no manual refetch ritual. With the SKYFE027 defaults wired, a successful mutation already invalidates
// every active query — so an `onSuccess` whose entire body is refetch/invalidate calls is the convention's
// pre-history surviving as cargo cult (the pilot hand-rolled it in 30 of 43 ViewModels; the 13 that forgot were
// the bug). Deleting it is the point: less ceremony per mutation, one fewer thing the next screen can forget.
// An `onSuccess` that does MORE than refetch (navigate, reset a form, hand off an id) is real behavior — never
// flagged. Warn-tier: it reveals redundancy, it does not gate.
module.exports = {
  meta: {
    type: "problem",
    docs: {
      description:
        "No onSuccess whose body only refetches/invalidates — the SKYFE027 mutation defaults already invalidate every active query on success; keep handlers only when they do more.",
    },
    messages: {
      ritual:
        "SKYFE028: redundant manual refetch — the app's mutation defaults (SKYFE027, lib/query.ts) already invalidate every active query on mutation success. Delete this `onSuccess`; keep a handler only when it does more than refetch.",
    },
  },
  create(context) {
    const f = context.filename.replace(/\\/g, "/");
    if (!isViewModel(f)) return {};
    const REFETCHISH = /^(refetch|invalidateQueries|resetQueries|refetchQueries)$/;
    const decls = new Map(); // name -> initializer (declared anywhere in the file)
    const candidates = [];
    // `useCallback(fn, deps)` wraps the ritual without changing it — analyze the wrapped fn.
    const unwrapInit = (init) =>
      init && init.type === "CallExpression" && init.callee.type === "Identifier" && init.callee.name === "useCallback"
        ? (init.arguments[0] ?? null)
        : init;
    const unwrapExpr = (expr) => {
      let e = expr;
      for (;;) {
        if (e && e.type === "UnaryExpression" && e.operator === "void") e = e.argument;
        else if (e && e.type === "AwaitExpression") e = e.argument;
        else if (e && e.type === "ChainExpression") e = e.expression;
        else return e;
      }
    };
    // A "refetch-ish" expression: a call to *.refetch()/queryClient.invalidateQueries(...) (any receiver), or a
    // call to a local name that itself resolves to a pure-refetch function (`onSuccess: () => invalidateSteps()`).
    const isRefetchCall = (expr, seen) => {
      const e = unwrapExpr(expr);
      if (!e || e.type !== "CallExpression") return false;
      const callee = e.callee;
      if (callee.type === "MemberExpression" && !callee.computed && callee.property.type === "Identifier")
        return REFETCHISH.test(callee.property.name);
      if (callee.type === "Identifier") return isPureRefetchName(callee.name, seen);
      return false;
    };
    const isPureRefetchFn = (fn, seen) => {
      if (!fn || (fn.type !== "ArrowFunctionExpression" && fn.type !== "FunctionExpression")) return false;
      if (fn.body.type !== "BlockStatement") return isRefetchCall(fn.body, seen);
      if (fn.body.body.length === 0) return false;
      return fn.body.body.every((s) => s.type === "ExpressionStatement" && isRefetchCall(s.expression, seen));
    };
    const isPureRefetchName = (name, seen) => {
      if (seen.has(name)) return false; // cycle guard
      seen.add(name);
      return isPureRefetchFn(unwrapInit(decls.get(name)), seen);
    };
    return {
      VariableDeclarator(node) {
        if (node.id.type === "Identifier" && node.init) decls.set(node.id.name, node.init);
      },
      Property(node) {
        if (node.computed) return;
        const key =
          node.key.type === "Identifier" ? node.key.name : node.key.type === "Literal" ? node.key.value : null;
        if (key === "onSuccess") candidates.push(node);
      },
      // Resolved at exit so a ritual referenced before its declaration is still traced.
      "Program:exit"() {
        for (const node of candidates) {
          const pure =
            node.value.type === "Identifier"
              ? isPureRefetchName(node.value.name, new Set())
              : isPureRefetchFn(unwrapInit(node.value), new Set());
          if (pure) context.report({ node, messageId: "ritual" });
        }
      },
    };
  },
};
