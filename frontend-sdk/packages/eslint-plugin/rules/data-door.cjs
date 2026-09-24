"use strict";

const { GENERATED_OPERATIONS, isViewModel, isGenerated, isInfraDataDoor, forbidImport } = require("../lib/shared.cjs");

// SKYFE002 — the ViewModel is the only data door: only *.viewModel.ts may import the generated client.
module.exports = {
  meta: {
    type: "problem",
    docs: { description: "Only a *.viewModel.ts may import the generated client." },
    messages: {
      offdoor:
        "SKYFE002: the generated client is the ViewModel's alone — import it only from a *.viewModel.ts (one data door).",
      laundered:
        "SKYFE002: re-exporting the generated client launders the data door — a helper that `export … from \"client.gen\"` hands every importer the client without ever naming it. The door is the ViewModel; don't re-export the client through anything else.",
    },
  },
  create(context) {
    const f = context.filename.replace(/\\/g, "/");
    if (isViewModel(f) || isGenerated(f) || isInfraDataDoor(f)) return {};
    const isTypeOnlyExport = (node) =>
      node.exportKind === "type" ||
      (node.specifiers?.length > 0 && node.specifiers.every((s) => s.exportKind === "type"));
    return {
      ...forbidImport(context, GENERATED_OPERATIONS, "offdoor"),
      // `export { useThing } from "@/client.gen/x"` / `export * from "@/client.gen/x"` — the import rule's
      // trivial bypass: no import statement, same access handed to every consumer.
      ExportNamedDeclaration(node) {
        if (node.source && !isTypeOnlyExport(node) && GENERATED_OPERATIONS.test(node.source.value))
          context.report({ node, messageId: "laundered" });
      },
      ExportAllDeclaration(node) {
        if (node.exportKind !== "type" && GENERATED_OPERATIONS.test(node.source.value))
          context.report({ node, messageId: "laundered" });
      },
    };
  },
};
