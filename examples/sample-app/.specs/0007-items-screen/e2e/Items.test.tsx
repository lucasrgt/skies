import type { ReactNode } from "react";
import { afterEach, describe, it, expect } from "vitest";
import { cleanup, render, renderHook, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useItemsModel } from "../../../frontend/web/src/items/Items.viewModel";
import { ItemsView } from "../../../frontend/web/src/items/Items.view";

// The Items screen: the ViewModel (the data door) against the real generated client, the HTTP layer an MSW stand-in
// (.specs/web.setup.ts), and the View rendered through its states.
function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

afterEach(cleanup);

describe("Items screen", () => {
  it("FM-1: the list resource starts in loading while it is fetched", () => {
    const { result } = renderHook(() => useItemsModel(), { wrapper });
    expect(result.current.state.items.status).toBe("loading");
  });

  it("FM-2: a list that settles empty renders the kit's empty state", async () => {
    render(<ItemsView />, { wrapper });
    expect(await screen.findByText("Nothing here yet")).toBeTruthy();
  });
});
