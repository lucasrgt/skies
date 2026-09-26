"use strict";

const { isInfraDataDoor, isRoute, authBoolName, returnsRedirect } = require("../lib/shared.cjs");

// SKYFE017 — a route guard branches its redirect on a tri-state SessionState, NEVER a raw `isAuthenticated`
// boolean. The boolean has no "still loading" — it is false while the session is in flight, so the guard fires its
// redirect before the answer settles (the canonical bounce-to-login). Branch on `session.status` instead, where
// `loading` is a distinct case you must handle. The read-side twin of SKYFE010 (a View routes state through the
// spine's union, not raw `isPending`). Scoped to route/guard files — where redirects live.
module.exports = {
  meta: {
    type: "problem",
    docs: {
      description:
        "A guard redirects on a tri-state SessionState (loading | authenticated | anonymous), not a raw isAuthenticated boolean (which reads 'still loading' as 'signed out' and bounces a not-yet-settled user to login).",
    },
    messages: {
      boolRedirect:
        "SKYFE017: don't redirect on a raw `{{name}}` boolean — it reads 'still loading' as 'signed out', bouncing a not-yet-settled user to login. Branch on a tri-state session: handle `loading` (defer), then `if (session.status === 'anonymous') return <Navigate…/>` (use the spine's SessionState).",
    },
  },
  create(context) {
    const f = context.filename.replace(/\\/g, "/");
    if (!isRoute(f) && !isInfraDataDoor(f)) return {};
    return {
      IfStatement(node) {
        // `if (!<authBool>) return <Navigate …/>` — the boolean-collapse redirect.
        if (node.test.type !== "UnaryExpression" || node.test.operator !== "!") return;
        const name = authBoolName(node.test.argument);
        if (name && returnsRedirect(node.consequent))
          context.report({ node: node.test, messageId: "boolRedirect", data: { name } });
      },
    };
  },
};
