import type { ReactNode } from "react";
import { afterEach, describe, it, expect } from "vitest";
import { cleanup, render, renderHook, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useItemsModel } from "./Items.viewModel";
import { ItemsView } from "./Items.view";

// Colocated tests: renderHook the ViewModel (the data door) against the real client (the HTTP layer is stubbed
// with MSW in vitest.setup.ts), and render the View through its states.
function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

afterEach(cleanup);

describe("Items", () => {
  it("starts its resource in loading while the list is fetched", () => {
    const { result } = renderHook(() => useItemsModel(), { wrapper });
    expect(result.current.state.items.status).toBe("loading");
  });

  it("renders the View without crashing", () => {
    const { container } = render(<ItemsView />, { wrapper });
    expect(container).toBeTruthy();
  });

  it("renders the empty branch through the kit when the list settles empty", async () => {
    render(<ItemsView />, { wrapper });
    expect(await screen.findByText("Nothing here yet")).toBeTruthy();
  });
});
