"use strict";

const { isTypeOnly } = require("../lib/shared.cjs");

// SKYFE004 — a ViewModel is render-agnostic: it renders no JSX and imports no `react-dom`. The ViewModel is the
// screen's logic, driven in a spec with `renderHook` and nothing else; the moment it returns an element or reaches
// for the DOM it can only be proven by rendering, and the View/ViewModel seam (the View renders, the ViewModel
// decides) blurs. The Flutter twin (SKYFL004) keeps Widget, BuildContext, and Material out of a ViewModel.
const VIEW_MODEL = /\.viewModel\.tsx?$/;
const RENDERING = /^react-dom($|\/)/;

module.exports = {
  meta: {
    type: "problem",
    docs: { description: "A ViewModel (*.viewModel.ts) renders no JSX and imports no react-dom." },
    messages: {
      jsx: "SKYFE004: a ViewModel renders nothing — return state and commands, and let the View (*.view.tsx) render this element.",
      dom: "SKYFE004: a ViewModel does not import `{{source}}` — it stays render-agnostic so its spec drives it with renderHook; DOM work belongs in the View or an injected port.",
    },
  },
  create(context) {
    const f = context.filename.replace(/\\/g, "/");
    if (!VIEW_MODEL.test(f)) return {};
    const reportJsx = (node) => context.report({ node, messageId: "jsx" });
    return {
      ImportDeclaration(node) {
        if (!isTypeOnly(node) && RENDERING.test(node.source.value))
          context.report({ node, messageId: "dom", data: { source: node.source.value } });
      },
      // Only the outermost element: one finding per rendered tree.
      JSXElement(node) {
        if (node.parent.type !== "JSXElement" && node.parent.type !== "JSXFragment") reportJsx(node);
      },
      JSXFragment(node) {
        if (node.parent.type !== "JSXElement" && node.parent.type !== "JSXFragment") reportJsx(node);
      },
    };
  },
};
