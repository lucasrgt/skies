import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// `@/` is the package's src/ (the same alias tsconfig.json declares), so a feature imports `@/ui`, `@/i18n`, and
// `@/client.gen/<api>` from any depth.
// The dev server's port is pinned: the API allows this origin (http://localhost:5173) in Development, so a
// silently moved port would turn every call into a CORS failure. Other environments list theirs in Cors:Origins.
export default defineConfig({
  plugins: [react()],
  server: { port: 5173, strictPort: true },
  resolve: { alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) } },
});
