"use strict";

const { isTest, isInfraDataDoor } = require("../lib/shared.cjs");

// SKYFE016 — the session is written through ONE seam (lib/session). The bug: token writes scattered across
// viewModels (login, signup, impersonate), each of which must REMEMBER to reset the `me` cache — and the one that
// forgets bounces the just-authenticated user back to login (a stale anonymous `me` error survives the sign-in).
// Centralizing the write in the seam pairs token + cache-reset by construction. Same "one door" shape as SKYFE002:
// only the seam may import the token setter; everywhere else goes through the seam's signIn/signOut.
module.exports = {
  meta: {
    type: "problem",
    docs: {
      description:
        "Write the session token through one seam (lib/session) — a scattered token write that forgets to reset the session cache bounces the just-authenticated user back to login.",
    },
    messages: {
      offdoor:
        "SKYFE016: write the session only through the seam — import the token setter (`{{name}}`) into `lib/session` and expose signIn/signOut, not here. A scattered token write that forgets to reset the `me` query bounces the just-authenticated user back to login.",
      storage:
        "SKYFE016: don't write the token to storage here (`{{call}}(\"{{key}}\", …)`) — that is a session write outside the seam, and it skips the `me`-cache reset the seam pairs with it. Call the seam's signIn/signOut instead; only `lib/session` touches token storage.",
    },
  },
  create(context) {
    const f = context.filename.replace(/\\/g, "/");
    if (isInfraDataDoor(f) || isTest(f)) return {}; // the seam (lib/session) legitimately writes; tests seed freely
    // A storage write keyed by a token-ish name is the same scattered session write as importing the setter —
    // the name-pattern door closes, the localStorage/AsyncStorage/SecureStore door must close with it.
    const TOKEN_KEY = /token|session|jwt|auth/i;
    const STORAGE = /^(localStorage|sessionStorage|AsyncStorage|SecureStore)$/;
    return {
      ImportDeclaration(node) {
        for (const s of node.specifiers) {
          if (s.type === "ImportSpecifier" && /^set(Access)?(Token|Session)$/.test(s.imported.name))
            context.report({ node: s, messageId: "offdoor", data: { name: s.imported.name } });
        }
      },
      CallExpression(node) {
        const callee = node.callee;
        if (callee.type !== "MemberExpression" || callee.computed) return;
        if (callee.property.type !== "Identifier" || !/^set(Item|ItemAsync)$/.test(callee.property.name)) return;
        const obj = callee.object;
        const root =
          obj.type === "Identifier"
            ? obj.name
            : obj.type === "MemberExpression" && obj.property.type === "Identifier"
              ? obj.property.name // window.localStorage
              : null;
        if (!root || !STORAGE.test(root)) return;
        const key = node.arguments[0];
        if (key && key.type === "Literal" && typeof key.value === "string" && TOKEN_KEY.test(key.value))
          context.report({
            node,
            messageId: "storage",
            data: { call: `${root}.${callee.property.name}`, key: key.value },
          });
      },
    };
  },
};
