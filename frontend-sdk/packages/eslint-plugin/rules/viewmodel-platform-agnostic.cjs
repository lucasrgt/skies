"use strict";

const { isViewModel } = require("../lib/shared.cjs");

// SKYFE009 — the ViewModel is platform-agnostic: no react-native / expo import. Platform capabilities
// (storage, navigation, push) are injected ports, not direct imports — so the ViewModel + the rest of the
// core (client, types) stay shareable web<->mobile and testable in Vitest (jsdom), with the View the only
// platform-specific layer.
module.exports = {
  meta: {
    type: "problem",
    docs: { description: "A *.viewModel.ts imports no react-native / expo (platform-agnostic core)." },
    messages: {
      platform:
        "SKYFE009: a ViewModel is platform-agnostic — no react-native/expo import. Inject platform capabilities as ports so the core stays shareable web↔mobile.",
    },
  },
  create(context) {
    const f = context.filename.replace(/\\/g, "/");
    if (!isViewModel(f)) return {};
    const platform = /^react-native($|\/|-)|^@react-native|^expo($|[-/])/;
    // Flag value AND type imports — an RN type leaks the platform into the agnostic core (a web client has
    // no react-native types), defeating the web↔mobile sharing the rule protects.
    return {
      ImportDeclaration(node) {
        if (platform.test(node.source.value)) context.report({ node, messageId: "platform" });
      },
    };
  },
};
