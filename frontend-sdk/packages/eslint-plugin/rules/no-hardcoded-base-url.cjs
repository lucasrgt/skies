"use strict";

const { isTest } = require("../lib/shared.cjs");

// SKYFE020 — the API base URL comes from CONFIGURATION, never a hardcoded host baked into the client's construction
// (`axios.create({ baseURL: "http://localhost:8080" })`). A baked literal can't follow dev/prod or a different
// port, so it silently 404s when the backend runs elsewhere — the pilot's "front says :8080, API runs on :5000"
// bug (the registered user bounced to login because `me` 404'd). Read it from env (`import.meta.env.VITE_API_URL`
// / `process.env.EXPO_PUBLIC_API_URL`) with a relative or env fallback; the backend pins its dev port in
// launchSettings so the two agree by construction. An env-fallback (`env.X ?? "…"`), a relative base (`""`/`/api`),
// and an injectable default (`client.defaults.baseURL = …`, overridden at boot by configureClient) all pass.
module.exports = {
  meta: {
    type: "problem",
    docs: {
      description:
        "The API base URL comes from configuration (env / a relative base / an injected default), not a hardcoded host baked into the client's construction — so dev/prod/ports don't drift and silently 404.",
    },
    messages: {
      hardcoded:
        "SKYFE020: don't hardcode the API base URL (`{{url}}`) in the client's construction — it can't follow dev/prod or a different port and silently 404s when the backend runs elsewhere. Read it from env (`import.meta.env.VITE_API_URL` / `process.env.EXPO_PUBLIC_API_URL`) with a relative or env fallback; the backend pins its dev port in launchSettings.",
    },
  },
  create(context) {
    const f = context.filename.replace(/\\/g, "/");
    if (isTest(f)) return {};
    return {
      Property(node) {
        if (node.computed) return;
        const key = node.key;
        const name = key.type === "Identifier" ? key.name : key.type === "Literal" ? key.value : null;
        if (name !== "baseURL" && name !== "baseUrl") return;
        // Only a BARE absolute-URL literal is the bug. An env-fallback (`env.X ?? "…"`, a LogicalExpression) or a
        // relative base ("" / "/api") is configuration, not a baked host.
        const v = node.value;
        if (v.type === "Literal" && typeof v.value === "string" && /^https?:\/\//.test(v.value))
          context.report({ node: v, messageId: "hardcoded", data: { url: v.value } });
      },
    };
  },
};
