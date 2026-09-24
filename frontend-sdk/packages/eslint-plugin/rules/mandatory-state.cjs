"use strict";

const { GENERATED_OPERATIONS, isViewModel, isTypeOnly } = require("../lib/shared.cjs");

// SKYFE007 — a ViewModel that exposes server data exposes its states: the spine's closed AsyncState (`toAsyncState`,
// `combineAsyncStates`), or, hand-rolled, a query's pending and error flags projected beside its data. A ViewModel
// that hands the View `query.data` and nothing else forgets a state, and the View renders a blank screen for a failed
// load or an empty list for one still in flight. SKYFE010 makes the View route the union through <Resource>; this is
// its ViewModel half. Like its Flutter twin (SKYFL007, "a server-backed ViewModel exposes AsyncState"), it asks it of
// the ViewModel file once, not of every read: a panel hydrating a form default from a query its screen already loads
// (cached, states handled there) is a secondary read, not a second resource.
//
// "Server data" is a generated hook's result read for its `data`; a mutation (its result used to `mutate`) is a
// command, not a resource, so a form-only ViewModel reading `mutation.data` as its success surface is not flagged.
// Calibrating on a real app: 84 findings per read became 1 per file, the one ViewModel exposing server data with no
// state at all; the rest projected `isPending`/`isError` by hand or read a query their screen loads.
const SPINE = new Set(["toAsyncState", "combineAsyncStates", "AsyncState"]);
const COMMAND = new Set(["mutate", "mutateAsync"]);
const LOADING = new Set(["isPending", "isLoading", "isFetching", "isFetched", "isSuccess", "status"]);
const FAILURE = new Set(["isError", "error", "isLoadingError", "status"]);
const has = (used, set) => [...used].some((u) => set.has(u));

module.exports = {
  meta: {
    type: "problem",
    docs: {
      description:
        "A ViewModel exposing a generated query's data exposes its loading and error states with it: AsyncState (toAsyncState / combineAsyncStates), or the query's pending and error flags.",
    },
    messages: {
      raw: "SKYFE007: `{{hook}}`'s data leaves this ViewModel without its loading and error states — project it with `toAsyncState(...)` (or `combineAsyncStates`) and expose the AsyncState, so the View renders every state through <Resource>.",
    },
  },
  create(context) {
    const f = context.filename.replace(/\\/g, "/");
    if (!isViewModel(f)) return {};
    const hooks = new Set();
    // A state is exposed when the spine appears or some query's pending and error flags are both read.
    let states = false;
    const queries = [];

    // What a hook result is used for: `x.data` / `{ data }` reads a resource, `x.mutate` / `{ mutate }` runs a command.
    const uses = (declarator) => {
      const found = new Set();
      if (declarator.id.type === "ObjectPattern") {
        for (const p of declarator.id.properties)
          if (p.type === "Property" && p.key.type === "Identifier") found.add(p.key.name);
        return found;
      }
      for (const variable of context.sourceCode.getDeclaredVariables(declarator))
        for (const ref of variable.references) {
          const parent = ref.identifier.parent;
          if (parent.type === "MemberExpression" && parent.object === ref.identifier && !parent.computed)
            found.add(parent.property.name);
        }
      return found;
    };

    return {
      ImportDeclaration(node) {
        if (node.source.value.includes("@skiesjs/react"))
          for (const s of node.specifiers) if (s.type === "ImportSpecifier" && SPINE.has(s.imported.name)) states = true;
        if (isTypeOnly(node) || !GENERATED_OPERATIONS.test(node.source.value.replace(/\\/g, "/"))) return;
        for (const s of node.specifiers)
          if (s.type === "ImportSpecifier" && s.importKind !== "type" && /^use[A-Z]/.test(s.imported.name))
            hooks.add(s.local.name);
      },
      Identifier(node) {
        if (SPINE.has(node.name)) states = true;
      },
      CallExpression(node) {
        if (node.callee.type !== "Identifier" || !hooks.has(node.callee.name)) return;
        const parent = node.parent;
        if (parent.type === "VariableDeclarator" && parent.init === node) {
          const used = uses(parent);
          if (has(used, LOADING) && has(used, FAILURE)) states = true;
          else if (used.has("data") && !has(used, COMMAND)) queries.push(node);
        } else if (parent.type === "MemberExpression" && parent.object === node && parent.property.name === "data") {
          queries.push(node);
        }
      },
      "Program:exit"() {
        if (states || queries.length === 0) return;
        context.report({ node: queries[0], messageId: "raw", data: { hook: queries[0].callee.name } });
      },
    };
  },
};
