"use strict";

const { isRoute, ID_PARAM, enclosingFunction, aliasesOf, hasPresenceGuard } = require("../lib/shared.cjs");

// SKYFE018 — a route that reads a REQUIRED id param must guard its absence with a declarative redirect. Hitting the
// route param-less (a bookmark, a stale/mis-wired link) otherwise renders a "ghost" screen bound to an empty id —
// the pilot's empty "Propriedade" thread. The fix is `if (!id) return <Navigate to={…} />` before the View; a throw
// on the same test (`throw notFound()`, an error boundary) or an `invariant(id)` also keeps the ghost off. Scoped to
// the loose `useParams()` reads, where a param is typed `string | undefined`: React Router's bare `useParams()` and
// TanStack Router's `useParams({ strict: false })`. A strict TanStack read (`useParams({ from })`,
// `Route.useParams()`) is guaranteed by the matched route and is not checked. Only id-shaped names count (optional
// filter params don't ghost).
module.exports = {
  meta: {
    type: "problem",
    docs: {
      description:
        "A route reading a required id param (a loose useParams()) must guard its absence with a declarative redirect — a param-less hit otherwise renders a ghost screen on an empty id.",
    },
    messages: {
      unguarded:
        "SKYFE018: the route reads `{{name}}` from useParams() but never guards its absence — a param-less hit (bookmark / stale link) renders a ghost screen on an empty id. Add `if (!{{name}}) return <Navigate to={…} />;` before rendering the View.",
    },
  },
  create(context) {
    const f = context.filename.replace(/\\/g, "/");
    if (!isRoute(f)) return {};
    // A read is loose when it passes no options (React Router) or opts out of strictness (TanStack).
    const isLoose = (call) => {
      if (call.arguments.length === 0) return true;
      const options = call.arguments[0];
      return (
        options.type === "ObjectExpression" &&
        options.properties.some(
          (p) =>
            p.type === "Property" &&
            p.key.type === "Identifier" &&
            p.key.name === "strict" &&
            p.value.type === "Literal" &&
            p.value.value === false,
        )
      );
    };
    return {
      VariableDeclarator(node) {
        // `useParams({ strict: false }) as { id?: string }` is the same loose read behind a cast.
        let init = node.init;
        while (init && (init.type === "TSAsExpression" || init.type === "TSSatisfiesExpression")) init = init.expression;
        if (!init || init.type !== "CallExpression") return;
        if (init.callee.type !== "Identifier" || init.callee.name !== "useParams") return;
        if (!isLoose(init) || node.id.type !== "ObjectPattern") return;
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
