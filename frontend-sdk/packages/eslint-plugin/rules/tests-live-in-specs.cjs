"use strict";

// SKYFE036 — tests live in a spec. Every test in a Skies app sits in a `.specs/<id>-<slug>/e2e/` folder, titled after
// the failure mode it covers (`test("FM-2: …")`), because a spec is the only place a test is tied to a written failure
// mode and to a receipt that saw it fail first. A test anywhere else was written as coverage: nothing says what it
// guards and nothing proved it can fail. An isolated system (a formatter, a reducer, a ViewModel) gets its own spec
// whose e2e/ holds isolated cases. The rule is about where tests live, never whether they exist.
//
// A test is a call to `test` / `it` / `describe` (or a declaring member of one: `test.describe`, `it.each([...])(…)`,
// `describe.skip`) imported from a test runner, or used as a global the way vitest/jest globals mode provides it. A
// runner's non-declaring members (`test.extend`, `test.use`, `test.beforeEach`, `test.step`) build fixtures and hooks,
// not cases, so a support file that extends the runner is not a test. An app's own re-export of the runner (Playwright's
// `export const test = base.extend(…)`, imported as `import { test } from "./support/fixtures"`) is recognized by the
// call's shape: a title and a body. A local function that merely shares the name is not a test. Reported once per
// file, at the first test.

const RUNNERS = /^(vitest|@playwright\/test|@jest\/globals|bun:test|node:test)$/;
const TEST_FUNCTIONS = new Set(["test", "it", "describe"]);
// The members that still declare a case or a suite; any other member (extend, use, hooks, step) does not.
const DECLARING = new Set([
  "only", "skip", "todo", "fixme", "fail", "fails", "each", "for", "concurrent", "sequential", "shuffle",
  "describe", "serial", "parallel", "runIf", "skipIf",
]);
const IN_SPEC = /(^|\/)\.specs\//;

/**
 * The identifier a test call hangs off (`it` in `it(…)`, `it.each([…])(…)`, `test.describe.only(…)`), or null when the
 * chain passes through a member that declares nothing (`test.extend(…)`, `test.beforeEach(…)`).
 */
function rootIdentifier(callee) {
  let node = callee;
  for (;;) {
    if (node.type === "CallExpression") node = node.callee;
    else if (node.type === "MemberExpression") {
      if (node.computed || node.property.type !== "Identifier" || !DECLARING.has(node.property.name)) return null;
      node = node.object;
    } else return node.type === "Identifier" ? node : null;
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

/** Whether a call reads as a case or a suite: a title (string or template) followed by a body function. */
function hasTestShape(call) {
  const [title, ...rest] = call.arguments;
  const titled = title && ((title.type === "Literal" && typeof title.value === "string") || title.type === "TemplateLiteral");
  return Boolean(titled) && rest.some((a) => a.type === "ArrowFunctionExpression" || a.type === "FunctionExpression");
}

/**
 * Whether `identifier` names a runner's test function: imported from a runner, an undeclared/configured global, or a
 * test-named import from the app's own module (a re-exported, extended runner) called with a title and a body.
 */
function isRunnerFunction(identifier, scope, call) {
  const variable = resolve(scope, identifier.name);
  if (!variable || variable.defs.length === 0) return TEST_FUNCTIONS.has(identifier.name);
  const def = variable.defs[0];
  if (def.type !== "ImportBinding" || def.node.type !== "ImportSpecifier") return false;
  const imported = def.node.imported.name ?? def.node.imported.value;
  if (!TEST_FUNCTIONS.has(imported)) return false;
  return RUNNERS.test(def.parent.source.value) || hasTestShape(call);
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
        if (!root || !isRunnerFunction(root, sourceCode.getScope(node), node)) return;
        reported = true;
        context.report({ node, messageId: "outsideSpec" });
      },
    };
  },
};
