"use strict";

const { GENERATED_OPERATIONS, isTest, isGenerated, isInfraDataDoor, isTypeOnly } = require("../lib/shared.cjs");

// SKYFE029 — refresh-one-door. The session-rotation credential (the httpOnly refresh cookie) is BURNED by
// parallel rotation: the backend's theft detection sees a spent token replayed and revokes the whole session family. So the Refresh slice has exactly ONE consumer surface — the
// session seam's single-flight bootstrapSession (lib/session), which the client seam's 401 interceptor calls
// through the injected setTokenRefresher — and a screen/viewModel that imports the refresh hook/operation, or
// hand-rolls a POST to a refresh route, is the second rotation path that one day runs in parallel with the
// first. Born from a pilot near-miss: a refresh bootstrap added to the session seam raced the client's 401
// interceptor merged the same week — two cold-load rotations, one burned family.
module.exports = {
  meta: {
    type: "problem",
    docs: {
      description:
        "The session refresh is rotated through one seam only (the client's single-flight interceptor or the session seam's gated bootstrap) — a second rotation path runs in parallel one day and trips the backend's refresh-theft detection.",
    },
    messages: {
      offdoor:
        "SKYFE029: don't consume the refresh operation (`{{name}}`) here — rotation has ONE door (the session seam's single-flight bootstrapSession, which the client's 401 interceptor calls via setTokenRefresher). A second rotation path eventually runs in parallel with the first, replays a spent token, and the backend's theft detection burns the whole session family.",
      raw:
        "SKYFE029: don't hand-roll a refresh call (`{{call}}`) here — rotation has ONE door (the session seam's single-flight bootstrapSession, which the client's 401 interceptor calls via setTokenRefresher). A parallel rotation replays a spent token and the backend's theft detection burns the whole session family.",
    },
  },
  create(context) {
    const f = context.filename.replace(/\\/g, "/");
    // The doors: the session/guards infra seams, the client seam itself, the generated client, and tests.
    const isClientSeam = /(^|\/)lib\/(skies-)?client(\.|\/)/.test(f);
    if (isInfraDataDoor(f) || isClientSeam || isGenerated(f) || isTest(f)) return {};
    const REFRESH_NAMES = /^(use)?refresh(accesstoken|token|session)?$/i;
    const REFRESH_SOURCE = new RegExp(`${GENERATED_OPERATIONS.source}|(^|/)lib/(skies-)?client`);
    return {
      ImportDeclaration(node) {
        if (isTypeOnly(node) || !REFRESH_SOURCE.test(node.source.value.replace(/\\/g, "/"))) return;
        for (const s of node.specifiers) {
          if (s.type === "ImportSpecifier" && s.importKind !== "type" && REFRESH_NAMES.test(s.imported.name))
            context.report({ node: s, messageId: "offdoor", data: { name: s.imported.name } });
        }
      },
      CallExpression(node) {
        const callee = node.callee;
        if (callee.type !== "MemberExpression" || callee.computed) return;
        if (callee.property.type !== "Identifier" || callee.property.name !== "post") return;
        const arg = node.arguments[0];
        if (arg && arg.type === "Literal" && typeof arg.value === "string" && /refresh/i.test(arg.value))
          context.report({ node, messageId: "raw", data: { call: `.post("${arg.value}")` } });
      },
    };
  },
};
