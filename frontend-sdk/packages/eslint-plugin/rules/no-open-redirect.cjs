"use strict";

const { isView, isRoute, walk } = require("../lib/shared.cjs");

// SKYFE022 — never navigate to a value that arrived in the URL. `router.replace(returnTo)` /
// `window.location.href = next` where the target derives from a route/search param is an open redirect: a
// crafted link sends the user (and their session-carrying browser) anywhere the attacker chose — the phishing
// primitive. The fix is an allowlist: map the param to a KNOWN in-app route (`const to = routes[returnTo] ??
// "/home"`) and navigate to the mapped value, never the raw param. The rule tracks the identifiers bound from
// useLocalSearchParams / useSearchParams / useSearch and flags any navigation whose argument references one.
module.exports = {
  meta: {
    type: "problem",
    docs: {
      description:
        "No navigation to a raw route/search param (open redirect) — map the param through an allowlist of known in-app routes first.",
    },
    messages: {
      openRedirect:
        "SKYFE022: `{{call}}` navigates to a value that arrived in the URL (`{{name}}`) — an open redirect: a crafted link sends the user anywhere. Map it through an allowlist of known routes (`routes[{{name}}] ?? \"/home\"`) and navigate to the mapped value.",
    },
  },
  create(context) {
    const f = context.filename.replace(/\\/g, "/");
    if (!isView(f) && !isRoute(f)) return {};
    // Names that carry URL-supplied values: the params object itself and the names destructured from it.
    const tainted = new Set();
    const PARAM_HOOKS = /^(useLocalSearchParams|useSearchParams|useSearch|useGlobalSearchParams)$/;
    const taintedIn = (expr) => {
      let hit = null;
      walk(expr, (n) => {
        if (n.type === "Identifier" && tainted.has(n.name)) {
          hit = n.name;
          return true;
        }
        return false;
      });
      return hit;
    };
    const report = (node, call, name) =>
      context.report({ node, messageId: "openRedirect", data: { call, name } });
    return {
      VariableDeclarator(node) {
        if (!node.init || node.init.type !== "CallExpression") return;
        if (node.init.callee.type !== "Identifier" || !PARAM_HOOKS.test(node.init.callee.name)) return;
        if (node.id.type === "Identifier") tainted.add(node.id.name);
        if (node.id.type === "ObjectPattern")
          for (const p of node.id.properties)
            if (p.type === "Property" && p.value.type === "Identifier") tainted.add(p.value.name);
        if (node.id.type === "ArrayPattern" && node.id.elements[0]?.type === "Identifier")
          tainted.add(node.id.elements[0].name); // useSearchParams() → [params]
      },
      CallExpression(node) {
        const callee = node.callee;
        if (callee.type !== "MemberExpression" || callee.computed) return;
        if (callee.property.type !== "Identifier") return;
        const method = callee.property.name;
        const isRouterNav =
          callee.object.type === "Identifier" &&
          callee.object.name === "router" &&
          /^(replace|push|navigate)$/.test(method);
        const isLocationNav =
          /^(assign|replace)$/.test(method) &&
          ((callee.object.type === "Identifier" && callee.object.name === "location") ||
            (callee.object.type === "MemberExpression" &&
              callee.object.property.type === "Identifier" &&
              callee.object.property.name === "location"));
        if (!isRouterNav && !isLocationNav) return;
        for (const arg of node.arguments) {
          const name = taintedIn(arg);
          if (name) return report(node, `${isRouterNav ? "router" : "location"}.${method}(…)`, name);
        }
      },
      AssignmentExpression(node) {
        // window.location.href = <param> / location.href = <param>
        const left = node.left;
        if (
          left.type === "MemberExpression" &&
          left.property.type === "Identifier" &&
          left.property.name === "href"
        ) {
          const name = taintedIn(node.right);
          if (name) report(node, "location.href = …", name);
        }
      },
    };
  },
};
