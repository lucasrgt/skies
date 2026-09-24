"use strict";

// SKYFE011 — every locale in a *.i18n.ts declares the same keys. A feature's copy lives as sibling catalogs
// (ptBR / esES / enUS …); a key added to one but not the others is a silent untranslated string at runtime. The
// rule compares the top-level key sets across the file's exported object literals and flags any catalog missing a
// key its siblings have. (Catalog assembly + the "no hardcoded string" half are the generator's / a later rule's
// job; this pins parity, the failure that actually ships.)
module.exports = {
  meta: {
    type: "problem",
    docs: { description: "Every locale catalog in a *.i18n.ts declares the same keys." },
    messages: {
      missing:
        "SKYFE011: i18n catalog `{{catalog}}` is missing key(s) {{keys}} that sibling locales declare — every locale must carry the same keys.",
    },
  },
  create(context) {
    const f = context.filename.replace(/\\/g, "/");
    if (!/\.i18n\.ts$/.test(f)) return {};
    // Keys are compared as FLATTENED paths ("empty.title"), so a key missing inside a nested group is caught
    // the same as a missing top-level key — nesting is layout, not a parity boundary.
    const keysOf = (objExpr, prefix = "", keys = new Set()) => {
      for (const p of objExpr.properties) {
        if (p.type !== "Property" || p.computed) continue;
        const k = p.key.type === "Identifier" ? p.key.name : p.key.type === "Literal" ? String(p.key.value) : null;
        if (k === null) continue;
        let value = p.value;
        while (value && value.type === "TSAsExpression") value = value.expression;
        if (value && value.type === "ObjectExpression") keysOf(value, `${prefix}${k}.`, keys);
        else keys.add(prefix + k);
      }
      return keys;
    };
    return {
      "Program:exit"(program) {
        const catalogs = [];
        for (const stmt of program.body) {
          const decl =
            stmt.type === "ExportNamedDeclaration" && stmt.declaration && stmt.declaration.type === "VariableDeclaration"
              ? stmt.declaration
              : stmt.type === "VariableDeclaration"
                ? stmt
                : null;
          if (!decl) continue;
          for (const d of decl.declarations) {
            let init = d.init;
            while (init && init.type === "TSAsExpression") init = init.expression; // unwrap `as const`
            if (init && init.type === "ObjectExpression" && d.id.type === "Identifier") {
              catalogs.push({ name: d.id.name, keys: keysOf(init), node: d });
            }
          }
        }
        if (catalogs.length < 2) return; // need >= 2 locales to compare
        const union = new Set();
        for (const c of catalogs) for (const k of c.keys) union.add(k);
        for (const c of catalogs) {
          const missing = [...union].filter((k) => !c.keys.has(k));
          if (missing.length) {
            context.report({
              node: c.node,
              messageId: "missing",
              data: { catalog: c.name, keys: missing.map((k) => `"${k}"`).join(", ") },
            });
          }
        }
      },
    };
  },
};
