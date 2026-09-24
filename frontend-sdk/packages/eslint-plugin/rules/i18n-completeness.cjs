"use strict";

// SKYFE011 — every locale in a *.i18n.ts declares the same keys. A feature's copy lives as sibling catalogs
// (ptBR / esES / enUS …); a key added to one but not the others is a silent untranslated string at runtime. The rule
// flags any catalog missing a key its siblings have. (Catalog assembly + the "no hardcoded string" half are the
// generator's / a later rule's job; this pins parity, the failure that actually ships.)
//
// A catalog is a LOCALE, recognized by its name: sibling top-level objects named like locales (`ptBR`, `en_US`, `en`),
// or the locale-keyed children of one object (`const messages = { "pt-BR": {…}, "en-US": {…} }`). Any other object in
// the file (an error-code map, a key table) is a lookup, not a locale, and is never compared: calibrating on a real
// app showed that treating every object as a catalog buried each file in noise and still missed the nested layout.
const LOCALE = /^(?:[a-z]{2}|[a-z]{2,3}[-_]?[A-Z]{2}|[a-z]{2}[-_][a-z]{2})$/;

/** Strip `as const` / `satisfies T` wrappers off an expression. */
function unwrap(node) {
  let n = node;
  while (n && (n.type === "TSAsExpression" || n.type === "TSSatisfiesExpression")) n = n.expression;
  return n;
}

/** The static name of an object property's key, or null for a computed or spread entry. */
function keyName(p) {
  if (p.type !== "Property" || p.computed) return null;
  if (p.key.type === "Identifier") return p.key.name;
  if (p.key.type === "Literal") return String(p.key.value);
  return null;
}

// Keys are compared as FLATTENED paths ("empty.title"), so a key missing inside a nested group is caught the same as a
// missing top-level key — nesting is layout, not a parity boundary.
function keysOf(objExpr, prefix = "", keys = new Set()) {
  for (const p of objExpr.properties) {
    const k = keyName(p);
    if (k === null) continue;
    const value = unwrap(p.value);
    if (value && value.type === "ObjectExpression") keysOf(value, `${prefix}${k}.`, keys);
    else keys.add(prefix + k);
  }
  return keys;
}

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
    const report = (group) => {
      if (group.length < 2) return; // need >= 2 locales to compare
      const union = new Set();
      for (const c of group) for (const k of c.keys) union.add(k);
      for (const c of group) {
        const missing = [...union].filter((k) => !c.keys.has(k));
        if (missing.length)
          context.report({
            node: c.node,
            messageId: "missing",
            data: { catalog: c.name, keys: missing.map((k) => `"${k}"`).join(", ") },
          });
      }
    };
    return {
      "Program:exit"(program) {
        const siblings = [];
        for (const stmt of program.body) {
          const decl =
            stmt.type === "ExportNamedDeclaration" && stmt.declaration && stmt.declaration.type === "VariableDeclaration"
              ? stmt.declaration
              : stmt.type === "VariableDeclaration"
                ? stmt
                : null;
          if (!decl) continue;
          for (const d of decl.declarations) {
            const init = unwrap(d.init);
            if (!init || init.type !== "ObjectExpression" || d.id.type !== "Identifier") continue;
            if (LOCALE.test(d.id.name)) {
              siblings.push({ name: d.id.name, keys: keysOf(init), node: d });
              continue;
            }
            // One object keyed by locale: its children are the catalogs.
            const entries = init.properties.map((p) => ({ p, name: keyName(p), value: unwrap(p.value) }));
            const localeKeyed =
              entries.length >= 2 &&
              entries.every((e) => e.name !== null && LOCALE.test(e.name) && e.value?.type === "ObjectExpression");
            if (localeKeyed)
              report(entries.map((e) => ({ name: `${d.id.name}["${e.name}"]`, keys: keysOf(e.value), node: e.p })));
          }
        }
        report(siblings);
      },
    };
  },
};
