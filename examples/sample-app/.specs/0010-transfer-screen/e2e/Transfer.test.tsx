import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { TransferView } from "../../../frontend/web/src/transfer/Transfer.view";

// The Transfer screen through its View and ViewModel, against the real client hook (the HTTP layer is an MSW
// stand-in, frontend-sdk/vitest.setup.ts: a transfer above 100 answers 422 insufficient funds).
function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

// RFC 4122-valid v4 UUIDs (zod's z.uuid() checks version/variant bits, not just the shape).
const SOURCE = "11111111-1111-4111-8111-111111111111";
const DESTINATION = "22222222-2222-4222-8222-222222222222";

function fill(from: string, to: string, amount: string) {
  fireEvent.change(screen.getByLabelText("From wallet"), { target: { value: from } });
  fireEvent.change(screen.getByLabelText("To wallet"), { target: { value: to } });
  fireEvent.change(screen.getByLabelText("Amount"), { target: { value: amount } });
}

function submit() {
  fireEvent.click(screen.getByRole("button", { name: "Transfer" }));
}

describe("Transfer screen", () => {
  it("FM-1: an empty submit is blocked with three field errors inside the Field anatomy", async () => {
    const wire = vi.spyOn(globalThis, "fetch");
    render(<TransferView />, { wrapper });
    submit();
    const alerts = await screen.findAllByRole("alert");
    expect(alerts).toHaveLength(3);
    expect(screen.getByRole("heading", { name: "Transfer" })).toBeTruthy();
    expect(wire).not.toHaveBeenCalled();
  });

  it("FM-2: the same wallet on both sides is reported on the destination and never sent", async () => {
    const wire = vi.spyOn(globalThis, "fetch");
    render(<TransferView />, { wrapper });
    fill(SOURCE, SOURCE, "10");
    submit();
    expect(await screen.findByText("Choose a different wallet to send to.")).toBeTruthy();
    expect(wire).not.toHaveBeenCalled();
  });

  it("FM-3: a valid submit announces while pending, then reaches the success surface", async () => {
    render(<TransferView />, { wrapper });
    fill(SOURCE, DESTINATION, "40");
    submit();
    await waitFor(() => expect(screen.getByRole("button").getAttribute("aria-busy")).toBe("true"));
    expect(await screen.findByText("Transfer complete")).toBeTruthy();
  });

  it("FM-4: a refused transfer surfaces as a role=alert block and keeps the form", async () => {
    render(<TransferView />, { wrapper });
    fill(SOURCE, DESTINATION, "500");
    submit();
    const alert = await screen.findByText("We couldn't complete the transfer. Check the balance and try again.");
    expect(alert.getAttribute("role")).toBe("alert");
    expect(screen.getByLabelText("From wallet")).toBeTruthy();
  });
});
