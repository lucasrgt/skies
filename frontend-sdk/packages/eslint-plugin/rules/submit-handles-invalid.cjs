"use strict";

const { isViewModel } = require("../lib/shared.cjs");

// SKYFE031 — handleSubmit always carries its invalid path. RHF's `handleSubmit(onValid)` without the second
// argument swallows a validation failure SILENTLY — and when the failing field sits off-screen (another
// tab/step of a big editor), the submit button goes completely mute: no mutation, no toast, no visible error
// (the pilot's "save isn't saving" prod bug — cep/lat/long failing on a hidden tab). SKYFE013/SKYFE027 surface a
// FAILED MUTATION; this failure happens BEFORE the mutation, so it was the family's real hole. The blessed fix
// is the spine's `submitOrReveal(form.handleSubmit, onValid, { onInvalid })` — it forces the surface and
// resolves the first invalid field so the shell can navigate to it; a hand-passed `onInvalid` also passes.
// Warn-tier on entry: a single-screen form whose inline field errors are all visible is a legitimate shape;
// promote to error if the primitive absorbs the common case.
module.exports = {
  meta: {
    type: "problem",
    docs: {
      description:
        "A ViewModel's handleSubmit(onValid) must also handle the invalid path — pass onInvalid or use the spine's submitOrReveal — so a validation failure (especially on an off-screen tab/step) is never a silent, mute submit button.",
    },
    messages: {
      silent:
        "SKYFE031: `handleSubmit` with only the valid path — a validation failure is swallowed silently, and with the failing field off-screen (another tab/step) the submit button goes mute: no mutation, no toast, nothing. Use the spine's `submitOrReveal(form.handleSubmit, onValid, { onInvalid })` (it forces the surface and resolves the first invalid field to navigate to), or pass the second argument: `handleSubmit(onValid, onInvalid)`.",
    },
  },
  create(context) {
    const f = context.filename.replace(/\\/g, "/");
    if (!isViewModel(f)) return {};
    return {
      CallExpression(node) {
        const callee = node.callee;
        const isBare = callee.type === "Identifier" && callee.name === "handleSubmit";
        const isMember =
          callee.type === "MemberExpression" &&
          !callee.computed &&
          callee.property.type === "Identifier" &&
          callee.property.name === "handleSubmit";
        if ((isBare || isMember) && node.arguments.length === 1)
          context.report({ node, messageId: "silent" });
      },
    };
  },
};
