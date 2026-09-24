"use strict";

const { isTest, walk } = require("../lib/shared.cjs");

// SKYFE030 — no `as never`/`as any`/`as unknown` on a navigation target. The cast exists for one reason: to
// silence the router's typed routes — and with them silenced, a drifted route literal compiles clean and 404s
// in prod (the pilot incident: server-minted route strings navigated via `router.push(x as never)`; two of the
// routes didn't exist in the app). The fix is never the cast: with typed routes on (expo-router
// `experiments.typedRoutes` / TanStack's generated route tree) a literal is compile-checked, and a dynamic path
// takes the typed `{ pathname, params }` object shape. Router-agnostic like its routing siblings: recognizes
// router.push/replace/navigate, a useNavigate() binding, and the declarative <Redirect href>/<Navigate to>/
// <Link href|to>. The rule is only half the gate — its config pair is typed routes being ON; without that, a
// removed cast merely degrades to `string`.
module.exports = {
  meta: {
    type: "problem",
    docs: {
      description:
        "No `as never`/`as any`/`as unknown` on a navigation target (imperative argument or declarative href/to) — the cast silences typed routes, and a silenced router lets a drifted route literal compile clean and 404 in prod.",
    },
    messages: {
      castNav:
        "SKYFE030: don't cast a navigation target (`as {{type}}` in `{{call}}`) — the cast silences typed routes, so a drifted/invalid route compiles clean and 404s in prod. Pass a typed route literal or the `{ pathname, params }` object (typed routes on: expo-router `experiments.typedRoutes` / TanStack's route tree); never a cast.",
      castHref:
        "SKYFE030: don't cast `{{attr}}` on <{{component}}> (`as {{type}}`) — the cast silences typed routes, so a drifted/invalid route compiles clean and 404s in prod. Pass a typed route literal or the `{ pathname, params }` object; never a cast.",
    },
  },
  create(context) {
    const f = context.filename.replace(/\\/g, "/");
    if (isTest(f)) return {};
    const SILENCERS = { TSNeverKeyword: "never", TSAnyKeyword: "any", TSUnknownKeyword: "unknown" };
    // The cast may wrap the whole argument or sit anywhere inside it (`{ pathname: p as never }`,
    // `x as unknown as Href`) — walk the subtree and surface the first silencing cast.
    const findSilencingCast = (root) => {
      let hit = null;
      walk(root, (n) => {
        if ((n.type === "TSAsExpression" || n.type === "TSTypeAssertion") && SILENCERS[n.typeAnnotation.type]) {
          hit = { node: n, type: SILENCERS[n.typeAnnotation.type] };
          return true;
        }
        return false;
      });
      return hit;
    };
    // Identifiers bound from `useNavigate()` (TanStack) — so a bare `navigate(... as never)` is recognized.
    const navigators = new Set();
    const NAV_COMPONENTS = /^(Redirect|Navigate|Link)$/;
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
        if (
          callee.type === "MemberExpression" &&
          !callee.computed &&
          callee.object.type === "Identifier" &&
          callee.object.name === "router" &&
          callee.property.type === "Identifier" &&
          /^(push|replace|navigate)$/.test(callee.property.name)
        )
          call = `router.${callee.property.name}(…)`;
        else if (callee.type === "Identifier" && navigators.has(callee.name)) call = `${callee.name}(…)`;
        if (!call) return;
        for (const arg of node.arguments) {
          const hit = findSilencingCast(arg);
          if (hit) return context.report({ node: hit.node, messageId: "castNav", data: { type: hit.type, call } });
        }
      },
      JSXAttribute(node) {
        if (node.name.type !== "JSXIdentifier" || !/^(href|to)$/.test(node.name.name)) return;
        const el = node.parent;
        if (el.type !== "JSXOpeningElement" || el.name.type !== "JSXIdentifier" || !NAV_COMPONENTS.test(el.name.name))
          return;
        if (!node.value || node.value.type !== "JSXExpressionContainer") return;
        const hit = findSilencingCast(node.value.expression);
        if (hit)
          context.report({
            node: hit.node,
            messageId: "castHref",
            data: { type: hit.type, attr: node.name.name, component: el.name.name },
          });
      },
    };
  },
};
