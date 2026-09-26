"use strict";

const { isTest, walk, identifierAppears } = require("../lib/shared.cjs");

// SKYFE032 — a <Controller> render that never reads `fieldState` gives a validated error NO surface on its
// field (the pilot's Description input: `render={({ field }) => …}` — its validation failure showed nowhere,
// not inline, not as a toast). The sibling of SKYFE031: that one guarantees the FORM-level surface, this one the
// FIELD-level one; together "a validation error always shows" holds by construction. Near-zero false positives:
// passing `error={fieldState.error?.message}` on a field without validation is inert. A deliberately non-inline
// surface must still expose the same error state explicitly. Warn-tier on entry, promoted alongside
// SKYFE031. Only an inline render function is analyzed — a referenced render component is visible in review.
module.exports = {
  meta: {
    type: "problem",
    docs: {
      description:
        "A <Controller> render prop must read fieldState (destructured or accessed) and surface the field's error — a render that only takes `field` leaves a validation failure with no surface on that field.",
    },
    messages: {
      blind:
        "SKYFE032: this <Controller{{name}}> render never reads `fieldState` — a validation error on the field has NO surface (no inline error under the control). Read it and pass the error through: `render={({ field, fieldState }) => <… error={fieldState.error?.message} />}`.",
    },
  },
  create(context) {
    const f = context.filename.replace(/\\/g, "/");
    if (isTest(f)) return {};
    return {
      JSXOpeningElement(node) {
        if (node.name.type !== "JSXIdentifier" || node.name.name !== "Controller") return;
        let render = null;
        let name = "";
        for (const attr of node.attributes) {
          if (attr.type !== "JSXAttribute" || attr.name.type !== "JSXIdentifier") continue;
          if (attr.name.name === "render" && attr.value && attr.value.type === "JSXExpressionContainer")
            render = attr;
          if (attr.name.name === "name" && attr.value && attr.value.type === "Literal")
            name = ` name="${attr.value.value}"`;
        }
        if (!render) return;
        const fn = render.value.expression;
        if (fn.type !== "ArrowFunctionExpression" && fn.type !== "FunctionExpression") return;
        // Destructured (`{ field, fieldState }`) or accessed (`props.fieldState`) both count as reading —
        // one identifier walk over params + body covers every spelling.
        if (!identifierAppears(fn, "fieldState"))
          context.report({ node: render, messageId: "blind", data: { name } });
      },
    };
  },
};
