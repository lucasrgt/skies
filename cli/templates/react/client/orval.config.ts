import { defineConfig } from "orval";

// The shipped convention config. Generates one typed TanStack Query hook per slice from the .NET API's
// build-time OpenAPI contract (no running server needed). The generated code is plumbing: never hand-edited,
// committed verbatim; all behavior lives above it, in the ViewModels. Regenerate with `skies g client`.
export default defineConfig({
  {{ name }}: {
    input: {
      target: "{{ contract }}",
      // Audience filter: webhooks/internal endpoints carry an skies:* tag (WithEndpointKind on the backend)
      // and never become a hook, so the client carries only what the app may call.
      filters: { mode: "exclude", tags: ["skies:asset", "skies:webhook", "skies:internal"] },
    },
    output: {
      mode: "split",
      target: "./src/client.gen/{{ name }}.ts",
      schemas: "./src/client.gen/model",
      client: "react-query",
      httpClient: "axios",
      // Never wipe the output folder: `skies g client` refuses to run while src/client.gen/ holds a file without
      // orval's header, and after a run removes only the generated files the contract no longer produces.
      clean: false,
      prettier: false,
      override: {
        mutator: { path: "./src/lib/skies-client.ts", name: "skiesClient" },
        // No hook-kind override on purpose: orval's verb-based default IS the convention — GET verbs
        // generate a read hook, write verbs (POST/PUT/PATCH/DELETE) generate a mutation hook with
        // { data: <Input> } variables. Do NOT add an override that forces both hook kinds on every
        // operation: on orval 8.20 that turns every endpoint into a read hook, silently stripping the
        // write path of its mutation shape (.mutate / isPending) so the deposit/form recipe cannot bind.
      },
    },
  },
});
