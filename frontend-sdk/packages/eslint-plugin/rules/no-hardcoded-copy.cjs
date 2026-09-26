"use strict";

const { isView } = require("../lib/shared.cjs");

// SKYFE014 — no hardcoded user-facing copy in a View (*.view.tsx only). Flags JSX text children that contain a
// letter and string-literal values of copy props (placeholder, label, title, aria-label, alt, …): both are visible
// or announced copy that must go through i18n (t()), so it lands in the catalog SKYFE011 keeps complete across
// locales. `{t("…")}` and variables are expressions, never literals, so they pass; data props (value, name, id,
// variant, role) are not copy and are never checked.
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
    // Props that carry user-facing copy; only string-literal values are flagged.
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
