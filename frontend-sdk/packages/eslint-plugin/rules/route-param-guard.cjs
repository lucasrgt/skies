"use strict";

const { isRoute, ID_PARAM, enclosingFunction, aliasesOf, hasPresenceGuard } = require("../lib/shared.cjs");

// SKYFE018 — a route that reads a REQUIRED id param must guard its absence with a declarative redirect. Hitting the
// route param-less (a bookmark, a stale/mis-wired link) otherwise renders a "ghost" screen bound to an empty id —
// the pilot's empty "Propriedade" thread. The fix is `if (!id) return <Redirect href={…} />` before the View. Scoped
// to expo-router's `useLocalSearchParams` (the one router where a path/search param can be absent at render; on
// TanStack a matched route guarantees its path param) and to id-shaped names (optional filter params don't ghost).
module.exports = {
  meta: {
    type: "problem",
    docs: {
      description:
        "A route reading a required id param (useLocalSearchParams) must guard its absence with a declarative redirect — a param-less hit otherwise renders a ghost screen on an empty id.",
    },
    messages: {
      unguarded:
        "SKYFE018: the route reads `{{name}}` from useLocalSearchParams but never guards its absence — a param-less hit (bookmark / stale link) renders a ghost screen on an empty id. Add `if (!{{name}}) return <Redirect href={…} />;` before rendering the View.",
    },
  },
  create(context) {
    const f = context.filename.replace(/\\/g, "/");
    if (!isRoute(f)) return {};
    return {
      VariableDeclarator(node) {
        if (!node.init || node.init.type !== "CallExpression") return;
        if (node.init.callee.type !== "Identifier" || node.init.callee.name !== "useLocalSearchParams") return;
        if (node.id.type !== "ObjectPattern") return;
        const fn = enclosingFunction(node);
        if (!fn) return;
        for (const prop of node.id.properties) {
          if (prop.type !== "Property" || prop.key.type !== "Identifier" || !ID_PARAM.test(prop.key.name)) continue;
          const local = prop.value.type === "Identifier" ? prop.value.name : prop.key.name;
          if (!hasPresenceGuard(fn, aliasesOf(fn, local)))
            context.report({ node: prop, messageId: "unguarded", data: { name: local } });
        }
      },
    };
  },
};
