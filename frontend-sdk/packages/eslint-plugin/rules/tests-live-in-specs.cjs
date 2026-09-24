"use strict";

// SKYFE036 — tests live in a spec. Every test in a Skies app sits in a `.specs/<id>-<slug>/e2e/` folder, titled after
// the failure mode it covers (`test("FM-2: …")`), because a spec is the only place a test is tied to a written failure
// mode and to a receipt that saw it fail first. A test anywhere else was written as coverage: nothing says what it
// guards and nothing proved it can fail. An isolated system (a formatter, a reducer, a ViewModel) gets its own spec
// whose e2e/ holds isolated cases. The rule is about where tests live, never whether they exist.
//
// A test is a call to `test` / `it` / `describe` (or a member of one: `test.describe`, `it.each([...])(…)`,
// `describe.skip`) imported from a test runner, or used as a global the way vitest/jest globals mode provides it.
// A local function that merely shares the name is not a test. Reported once per file, at the first test.

const RUNNERS = /^(vitest|@playwright\/test|@jest\/globals|bun:test)$/;
const TEST_FUNCTIONS = new Set(["test", "it", "describe"]);
const IN_SPEC = /(^|\/)\.specs\//;

/** The identifier a test call hangs off: `it` in `it(…)`, `it.each([…])(…)`, `test.describe.only(…)`. */
function rootIdentifier(callee) {
  let node = callee;
  for (;;) {
    if (node.type === "CallExpression") node = node.callee;
    else if (node.type === "MemberExpression") node = node.object;
    else return node.type === "Identifier" ? node : null;
  }
}

/** The variable `name` resolves to from `scope`, or null when nothing declares it (an implicit global). */
function resolve(scope, name) {
  for (let current = scope; current; current = current.upper) {
    const variable = current.set.get(name);
    if (variable) return variable;
  }
  return null;
}

/** Whether `identifier` names a runner's test function: imported from a runner, or an undeclared/configured global. */
function isRunnerFunction(identifier, scope) {
  const variable = resolve(scope, identifier.name);
  if (!variable || variable.defs.length === 0) return TEST_FUNCTIONS.has(identifier.name);
  const def = variable.defs[0];
  if (def.type !== "ImportBinding" || def.node.type !== "ImportSpecifier") return false;
  const imported = def.node.imported.name ?? def.node.imported.value;
  return RUNNERS.test(def.parent.source.value) && TEST_FUNCTIONS.has(imported);
}

module.exports = {
  meta: {
    type: "problem",
    docs: { description: "Tests live in a spec: a test outside .specs/<id>-<slug>/e2e/ is flagged." },
    messages: {
      outsideSpec:
        "SKYFE036: tests live in a spec — move this file's cases into .specs/<id>-<slug>/e2e/ and title each after the failure mode it covers (`test(\"FM-n: …\")`). An isolated system gets its own spec.",
    },
  },
  create(context) {
    const f = context.filename.replace(/\\/g, "/");
    if (IN_SPEC.test(f)) return {};
    const sourceCode = context.sourceCode ?? context.getSourceCode();
    let reported = false;
    return {
      CallExpression(node) {
        if (reported) return;
        // `it.each([…])(…)` is one test: judge the outermost call only.
        if (node.parent.type === "CallExpression" && node.parent.callee === node) return;
        const root = rootIdentifier(node.callee);
        if (!root || !isRunnerFunction(root, sourceCode.getScope(node))) return;
        reported = true;
        context.report({ node, messageId: "outsideSpec" });
      },
    };
  },
};
