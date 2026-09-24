"use strict";

const { isView, isRoute, inEffect } = require("../lib/shared.cjs");

// SKYFE015 — no imperative redirect inside useEffect. A redirect-on-state belongs in a DECLARATIVE `<Navigate>`
// returned from render (TanStack Router and React Router both ship one), never an effect: an effect runs AFTER
// paint and re-fires on every re-render, so the source screen flashes before the redirect and a guard whose effect
// re-triggers a refetch can loop (the pilot shipped that loop twice: Splash, then ChooseRole + 5 screens). The fix
// is `if (<terminal state>) return <Navigate to={…} />`. Recognizes both idioms — `router.navigate(...)` on a
// router instance and a `navigate(...)` bound from `useNavigate()`. An imperative navigation on a USER action stays
// allowed (a completion, not a render loop). Scoped to the navigating layer.
module.exports = {
  meta: {
    type: "problem",
    docs: {
      description:
        "No imperative redirect (router.navigate / a useNavigate() call) inside useEffect — redirect declaratively with <Navigate> from render (an effect runs after paint and re-fires every render: a flash at best, a navigation/refetch loop at worst).",
    },
    messages: {
      effectReplace:
        "SKYFE015: no imperative redirect (`{{call}}`) inside useEffect — it runs after render and re-fires on every re-render (a flash at best; a navigation/refetch loop when the effect re-triggers a guard's query). Redirect declaratively instead: `if (<terminal state>) return <Navigate to={…} />;`.",
    },
  },
  create(context) {
    const f = context.filename.replace(/\\/g, "/");
    if (!isView(f) && !isRoute(f)) return {};
    // Identifiers bound from `useNavigate()` — so a bare `navigate(...)` in an effect is recognized.
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
        let call = null;
        // A router instance (TanStack's useRouter(), React Router's data router): router.navigate(...)
        if (
          callee.type === "MemberExpression" &&
          !callee.computed &&
          callee.object.type === "Identifier" &&
          callee.object.name === "router" &&
          callee.property.type === "Identifier" &&
          callee.property.name === "navigate"
        )
          call = "router.navigate";
        // navigate({ to }) / navigate("/x") where navigate = useNavigate()
        else if (callee.type === "Identifier" && navigators.has(callee.name)) call = `${callee.name}(...)`;
        // Flag only inside an effect. A user-action navigation is a completion, not a render loop.
        if (call && inEffect(node)) context.report({ node, messageId: "effectReplace", data: { call } });
      },
    };
  },
};
