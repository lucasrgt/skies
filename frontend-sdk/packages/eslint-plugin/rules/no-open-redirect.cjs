"use strict";

const { isView, isRoute } = require("../lib/shared.cjs");

// SKYFE022 — never navigate to a value that arrived in the URL. `navigate({ to: returnTo })` /
// `window.location.href = next` where the target derives from a route/search param is an open redirect: a
// crafted link sends the user (and their session-carrying browser) anywhere the attacker chose — the phishing
// primitive. The fix is an allowlist: map the param to a KNOWN in-app route (`const to = routes[returnTo] ??
// "/home"`) and navigate to the mapped value, never the raw param. The rule tracks the identifiers bound from
// useSearchParams (React Router) / useSearch or `Route.useSearch()` (TanStack Router) and flags any navigation whose
// TARGET references one: `router.navigate(…)`, a `useNavigate()` binding, `<Navigate to>`, `location.assign/replace(…)`,
// or `location.href = …`. Only the target counts (the first argument, or its `to`/`href`); a lookup key or a
// conditional's test is the allowlist at work, not a leak.
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
    const PARAM_HOOKS = /^(useSearchParams|useSearch)$/;
    // Identifiers bound from `useNavigate()`, so `navigate({ to: next })` is a navigation too.
    const navigators = new Set();
    // The URL-supplied name that flows into a navigation TARGET, or null. A lookup key (`ROUTES[next]`) is the
    // allowlist itself, and a conditional's test only picks between its branches, so neither taints the result.
    const taintedIn = (expr) => {
      if (!expr) return null;
      switch (expr.type) {
        case "Identifier":
          return tainted.has(expr.name) ? expr.name : null;
        case "MemberExpression":
          return taintedIn(expr.object);
        case "ConditionalExpression":
          return taintedIn(expr.consequent) ?? taintedIn(expr.alternate);
        case "TSAsExpression":
        case "TSNonNullExpression":
        case "TSSatisfiesExpression":
          return taintedIn(expr.expression);
        default:
          for (const key of Object.keys(expr)) {
            if (key === "parent") continue;
            const children = Array.isArray(expr[key]) ? expr[key] : [expr[key]];
            for (const child of children) {
              const hit = child && typeof child.type === "string" ? taintedIn(child) : null;
              if (hit) return hit;
            }
          }
          return null;
      }
    };
    // Where a navigation goes: the first argument, or its `to` / `href` when it is an options object. The rest of the
    // call (`search`, `params`, `state`, `replace`) rides along to an in-app route and is not the target: forwarding
    // `?redirect=` from login to register is how the allowlisted hop survives, not an open redirect.
    const targetOf = (call) => {
      const first = call.arguments[0];
      if (!first || first.type !== "ObjectExpression") return first ?? null;
      const to = first.properties.find(
        (p) => p.type === "Property" && !p.computed && p.key.type === "Identifier" && /^(to|href)$/.test(p.key.name),
      );
      return to ? to.value : null;
    };
    const report = (node, call, name) =>
      context.report({ node, messageId: "openRedirect", data: { call, name } });
    return {
      VariableDeclarator(node) {
        if (!node.init || node.init.type !== "CallExpression") return;
        // `useSearch()` and TanStack's file-route `Route.useSearch()` are the same read.
        const callee = node.init.callee;
        const hook =
          callee.type === "Identifier"
            ? callee.name
            : callee.type === "MemberExpression" && !callee.computed && callee.property.type === "Identifier"
              ? callee.property.name
              : null;
        if (!hook) return;
        if (hook === "useNavigate" && node.id.type === "Identifier") navigators.add(node.id.name);
        if (!PARAM_HOOKS.test(hook)) return;
        if (node.id.type === "Identifier") tainted.add(node.id.name);
        if (node.id.type === "ObjectPattern")
          for (const p of node.id.properties)
            if (p.type === "Property" && p.value.type === "Identifier") tainted.add(p.value.name);
        if (node.id.type === "ArrayPattern" && node.id.elements[0]?.type === "Identifier")
          tainted.add(node.id.elements[0].name); // useSearchParams() → [params]
      },
      CallExpression(node) {
        const callee = node.callee;
        if (callee.type === "Identifier" && navigators.has(callee.name)) {
          const name = taintedIn(targetOf(node));
          if (name) report(node, `${callee.name}(…)`, name);
          return;
        }
        if (callee.type !== "MemberExpression" || callee.computed) return;
        if (callee.property.type !== "Identifier") return;
        const method = callee.property.name;
        const isRouterNav =
          callee.object.type === "Identifier" &&
          callee.object.name === "router" &&
          method === "navigate";
        const isLocationNav =
          /^(assign|replace)$/.test(method) &&
          ((callee.object.type === "Identifier" && callee.object.name === "location") ||
            (callee.object.type === "MemberExpression" &&
              callee.object.property.type === "Identifier" &&
              callee.object.property.name === "location"));
        if (!isRouterNav && !isLocationNav) return;
        const name = taintedIn(isRouterNav ? targetOf(node) : node.arguments[0]);
        if (name) report(node, `${isRouterNav ? "router" : "location"}.${method}(…)`, name);
      },
      // The declarative twin: `<Navigate to={search.next} />`.
      JSXOpeningElement(node) {
        if (node.name.type !== "JSXIdentifier" || node.name.name !== "Navigate") return;
        const to = node.attributes.find((a) => a.type === "JSXAttribute" && a.name.name === "to");
        if (!to || !to.value || to.value.type !== "JSXExpressionContainer") return;
        const name = taintedIn(to.value.expression);
        if (name) report(node, "<Navigate to={…}>", name);
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
