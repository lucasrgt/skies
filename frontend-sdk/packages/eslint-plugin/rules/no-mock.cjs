"use strict";

const { MOCKS, isTest, forbidImport } = require("../lib/shared.cjs");

// SKYFE003 — no mock/fixture/MSW import in production code (only under *.test.*).
module.exports = {
  meta: {
    type: "problem",
    docs: { description: "No mock/fixture/MSW import outside tests." },
    messages: {
      mock: "SKYFE003: no mock/fixture/MSW in production code — mocks live only under *.test.* (wired, not mocked).",
    },
  },
  create(context) {
    const f = context.filename.replace(/\\/g, "/");
    if (isTest(f)) return {};
    return forbidImport(context, MOCKS, "mock");
  },
};
