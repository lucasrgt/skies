"use strict";

const { isTest } = require("../lib/shared.cjs");

// SKYFE021 — no dangerouslySetInnerHTML outside one audited seam. React's JSX escapes text by construction;
// dangerouslySetInnerHTML is the single opt-out, and server/user-influenced HTML through it is XSS. If the app
// truly renders rich HTML (a CMS body), that rendering lives in ONE seam (lib/html) where the sanitizer is
// wired and reviewable — the same one-door shape as SKYFE002/SKYFE016. Everywhere else the prop is flagged.
module.exports = {
  meta: {
    type: "problem",
    docs: {
      description:
        "No dangerouslySetInnerHTML outside the lib/html seam — JSX escapes by construction; raw HTML is the XSS door and belongs behind one audited, sanitizing seam.",
    },
    messages: {
      rawHtml:
        "SKYFE021: no dangerouslySetInnerHTML here — JSX already escapes; raw HTML is the XSS door. If the app renders rich HTML, do it in ONE seam (lib/html) with the sanitizer wired, and use that component.",
    },
  },
  create(context) {
    const f = context.filename.replace(/\\/g, "/");
    if (isTest(f) || /(^|\/)lib\/html(\.|\/)/.test(f)) return {};
    return {
      JSXAttribute(node) {
        if (node.name.type === "JSXIdentifier" && node.name.name === "dangerouslySetInnerHTML")
          context.report({ node, messageId: "rawHtml" });
      },
    };
  },
};
