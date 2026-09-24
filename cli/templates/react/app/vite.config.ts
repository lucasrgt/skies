import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// `@/` is the package's src/ (the same alias tsconfig.json declares), so a feature imports `@/ui`, `@/i18n`, and
// `@/client.gen/<api>` from any depth.
export default defineConfig({
  plugins: [react()],
  resolve: { alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) } },
});
