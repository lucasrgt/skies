"use strict";

const { isView } = require("../lib/shared.cjs");

// SKYFE010 — a View routes loading/error/empty through the spine (<Resource> over an AsyncState), never raw
// react-query booleans. The moment a View hand-reads isPending/isError it has taken on state handling the spine
// exists to make exhaustive — and the forgotten branch (no empty state, no error UI) is exactly what slips
// through. So the booleans are the ViewModel's (it projects them via toAsyncState); the View consumes the union.
module.exports = {
  meta: {
    type: "problem",
    docs: { description: "A View handles async state through <Resource>, not raw isPending/isError." },
    messages: {
      raw:
        "SKYFE010: a View routes loading/error/empty through <Resource> (the spine), not raw `{{name}}` — expose the resource as AsyncState in the ViewModel and render it via <Resource>, so every state is handled by construction.",
    },
  },
  create(context) {
    const f = context.filename.replace(/\\/g, "/");
    if (!isView(f)) return {};
    const RAW = /^(isPending|isLoading|isError|isFetching|isRefetching|isSuccess)$/;
    return {
      // `query.isPending`
      MemberExpression(node) {
        if (!node.computed && node.property.type === "Identifier" && RAW.test(node.property.name)) {
          context.report({ node: node.property, messageId: "raw", data: { name: node.property.name } });
        }
      },
      // `const { isError } = useFooModel()`
      Property(node) {
        if (
          node.parent.type === "ObjectPattern" &&
          !node.computed &&
          node.key.type === "Identifier" &&
          RAW.test(node.key.name)
        ) {
          context.report({ node: node.key, messageId: "raw", data: { name: node.key.name } });
        }
      },
    };
  },
};
