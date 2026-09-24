"use strict";

const { isView, isRoute, inEffect } = require("../lib/shared.cjs");

// SKYFE015 — no imperative redirect inside useEffect. A redirect-on-state belongs in a DECLARATIVE element returned
// from render (<Redirect> on expo-router, <Navigate> on TanStack), never an effect: an effect runs AFTER paint and
// re-fires on every re-render. On expo-router web it is catastrophic — the router FREEZES the source screen instead
// of unmounting it, so the effect loops (replace -> remount target -> refetch a guard's my-X 404 -> re-render ->
// replace …) into an infinite navigation/refetch loop that crashes the screen (shipped twice in the pilot: Splash,
// then ChooseRole + 5 screens). On TanStack it is "merely" a post-paint flash + redundant nav. Either way the fix
// is the same: `if (<terminal state>) return <Redirect/Navigate … />`. Recognizes both idioms — `router.replace` /
// `router.navigate` (a router instance) and `navigate(...)` bound from TanStack's `useNavigate()`. Imperative
// push/back on a USER action stay allowed (completions, not render loops). Scoped to the navigating layer.
module.exports = {
  meta: {
    type: "problem",
    docs: {
      description:
        "No imperative redirect (router.replace / router.navigate / a useNavigate() call) inside useEffect — redirect declaratively with <Redirect>/<Navigate> from render (an effect runs after paint and re-fires every render: a flash on TanStack, an infinite loop on expo-router web).",
    },
    messages: {
      effectReplace:
        "SKYFE015: no imperative redirect (`{{call}}`) inside useEffect — it runs after render and re-fires on every re-render (a flash on TanStack; on expo-router web an infinite navigation/refetch loop that crashes the screen). Redirect declaratively instead: `if (<terminal state>) return <Redirect href={…} />;` (expo) / `<Navigate to={…} />` (TanStack).",
    },
  },
  create(context) {
    const f = context.filename.replace(/\\/g, "/");
    if (!isView(f) && !isRoute(f)) return {};
    // Identifiers bound from `useNavigate()` (TanStack) — so a bare `navigate(...)` in an effect is recognized.
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
        // expo-router / a router instance: router.replace(...) / router.navigate(...)
        if (
          callee.type === "MemberExpression" &&
          !callee.computed &&
          callee.object.type === "Identifier" &&
          callee.object.name === "router" &&
          callee.property.type === "Identifier" &&
          (callee.property.name === "replace" || callee.property.name === "navigate")
        )
          call = `router.${callee.property.name}`;
        // TanStack: navigate({ to }) where navigate = useNavigate()
        else if (callee.type === "Identifier" && navigators.has(callee.name)) call = `${callee.name}(...)`;
        // Flag only inside an effect. A user-action push/back/replace is a completion, not a render loop.
        if (call && inEffect(node)) context.report({ node, messageId: "effectReplace", data: { call } });
      },
    };
  },
};
