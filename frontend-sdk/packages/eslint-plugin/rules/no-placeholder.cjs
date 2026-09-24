"use strict";

const { isTest, isGenerated } = require("../lib/shared.cjs");

// SKYFE023 — no unfinished placeholder in production code: a `TODO`/`FIXME`/`HACK`/`XXX`/"wire later" comment, a
// `@ts-expect-error`/`@ts-ignore` that silences the compiler (the check that makes "wired" decidable), or a
// `throw new Error("not implemented")` stub. Each is the visible trace of "almost done" shipped as done, the failure
// the frontend harness exists to catch. Warning tier: it is a signal for review, not an architecture boundary, and
// an app mid-migration legitimately carries some. The Flutter twin (SKYFL023) reads the same markers and
// `UnimplementedError`.
// The markers are the conventional uppercase ones: a lowercase "todo" is a word (Portuguese "all": "em todo o app"),
// found calibrating on a real app.
const MARKER = /\b(TODO|FIXME|HACK|XXX)\b|\b[Ww]ire later\b/;
const SILENCER = /@ts-(expect-error|ignore)\b/;
const STUB = /\bnot (yet )?implemented\b/i;

module.exports = {
  meta: {
    type: "suggestion",
    docs: {
      description:
        "No unfinished placeholder in production code: TODO/FIXME/HACK/XXX or 'wire later' comments, @ts-expect-error/@ts-ignore, or a 'not implemented' stub.",
    },
    messages: {
      marker: "SKYFE023: an unfinished-work marker (`{{marker}}`) ships in production code — finish it, or track it outside the code.",
      silencer:
        "SKYFE023: `{{marker}}` silences the compiler that makes 'wired' decidable — fix the type (regenerate the client if the contract moved) instead of hiding the error.",
      stub: "SKYFE023: a 'not implemented' stub ships in production code — implement it or remove the path that reaches it.",
    },
  },
  create(context) {
    const f = context.filename.replace(/\\/g, "/");
    if (isTest(f) || isGenerated(f) || f.includes("/.specs/")) return {};
    return {
      Program() {
        for (const comment of context.sourceCode.getAllComments()) {
          const silencer = comment.value.match(SILENCER);
          const marker = comment.value.match(MARKER);
          if (silencer)
            context.report({ loc: comment.loc, messageId: "silencer", data: { marker: silencer[0] } });
          else if (marker) context.report({ loc: comment.loc, messageId: "marker", data: { marker: marker[0] } });
        }
      },
      ThrowStatement(node) {
        const arg = node.argument;
        if (!arg || arg.type !== "NewExpression" || arg.callee.type !== "Identifier" || arg.callee.name !== "Error")
          return;
        const message = arg.arguments[0];
        if (message && message.type === "Literal" && typeof message.value === "string" && STUB.test(message.value))
          context.report({ node, messageId: "stub" });
      },
    };
  },
};
