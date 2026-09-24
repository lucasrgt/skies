"use strict";

const { GENERATED_OPERATIONS, DATA_LIBS, isView, isTypeOnly } = require("../lib/shared.cjs");

// SKYFE001 — a View renders only; it owns no data access. Server data comes from its ViewModel.
module.exports = {
  meta: {
    type: "problem",
    docs: { description: "A View (*.view.tsx) imports no data layer; it consumes its ViewModel." },
    messages: {
      impure:
        "SKYFE001: a View renders only — get server data from its ViewModel (*.viewModel.ts), not the client/axios/react-query.",
    },
  },
  create(context) {
    const f = context.filename.replace(/\\/g, "/");
    if (!isView(f)) return {};
    return {
      ImportDeclaration(node) {
        if (isTypeOnly(node)) return; // contract types are fine in a View; only data access is not
        const src = node.source.value;
        if (GENERATED_OPERATIONS.test(src) || DATA_LIBS.test(src)) {
          context.report({ node, messageId: "impure" });
        }
      },
    };
  },
};
