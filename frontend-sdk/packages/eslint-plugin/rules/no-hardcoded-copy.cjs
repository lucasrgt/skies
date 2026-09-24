"use strict";

const { isView } = require("../lib/shared.cjs");

// SKYFE014 — no hardcoded user-facing copy in a View. Targets JSX text children (the `>text<` between tags) that
// contain a letter — almost always visible copy that must go through i18n (t()), so it lands in the catalog
// SKYFE011 then keeps complete across locales. Deliberately scoped to JSXText (high signal, near-zero false
// positives): `{t("…")}` is an expression (not text) so it's never flagged, and className / testID / name /
// variant are attributes (not children) so they're never flagged either. The trade-off is COVERAGE not noise —
// copy hidden in props (placeholder=…) or variables is NOT caught here (a Phase-2 copy-prop whitelist can add
// it). Warn-first: it surfaces hardcoded strings without crying wolf.
module.exports = {
  meta: {
    type: "problem",
    docs: { description: "A View has no hardcoded user-facing text — JSX text goes through i18n (t())." },
    messages: {
      hardcoded:
        'SKYFE014: user-facing text must go through i18n — wrap "{{text}}" in t() (no hardcoded copy in a View).',
    },
  },
  create(context) {
    const f = context.filename.replace(/\\/g, "/");
    if (!isView(f)) return {};
    // Phase 2: props that carry user-facing copy. Only STRING-LITERAL values are flagged — `{t()}` and variables
    // are JSXExpressionContainers, not literals, so they're never touched. `value`/`name`/`id`/`variant`/`role`
    // are deliberately NOT here (they're data/ids/enums, not copy).
    const COPY_PROPS = new Set([
      "placeholder", "label", "title", "subtitle", "heading", "description", "message",
      "helperText", "caption", "errorMessage", "emptyTitle", "emptyDescription",
      "aria-label", "alt",
    ]);
    const flag = (node, raw) => {
      const text = raw.trim();
      if (!text || !/[a-zA-Z]/.test(text)) return; // whitespace / numbers / punctuation only
      context.report({
        node,
        messageId: "hardcoded",
        data: { text: text.length > 40 ? `${text.slice(0, 40)}…` : text },
      });
    };
    return {
      JSXText(node) {
        flag(node, node.value);
      },
      JSXAttribute(node) {
        if (node.name.type !== "JSXIdentifier" || !COPY_PROPS.has(node.name.name)) return;
        const v = node.value;
        if (!v || v.type !== "Literal" || typeof v.value !== "string") return; // only literal copy
        flag(v, v.value);
      },
    };
  },
};
