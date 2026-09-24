import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider } from "@tanstack/react-router";
import i18n from "@/i18n";
import { configureClient } from "@/lib/skies-client";
import { createQueryClient } from "@/lib/query";
import { router } from "@/routes/router";

// The boot seams, wired once: the API base URL from configuration (VITE_API_URL, see .env.example), the
// QueryClient with the write-side defaults, and the router. A toast library plugs into lib/feedback.ts
// (`wireFeedback`); until then failures surface on the console.
configureClient(import.meta.env.VITE_API_URL ?? "");

const queryClient = createQueryClient({
  saved: () => i18n.t("shell:saved"),
  failed: () => i18n.t("shell:failed"),
});

const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <QueryClientProvider client={queryClient}>
        <RouterProvider router={router} />
      </QueryClientProvider>
    </StrictMode>,
  );
}
