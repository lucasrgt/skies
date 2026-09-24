import type { ReactNode } from "react";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, renderHook, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useDepositModel } from "./Deposit.viewModel";
import { DepositView } from "./Deposit.view";

// Form behavior: validation blocks inside the Field anatomy, the command failure surfaces as a role=alert block,
// the submit announces while pending, and success replaces the form (a routed app would <Redirect> — SKYFE015).
function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}

afterEach(cleanup);

// RFC 4122-valid v4 UUIDs (zod's z.uuid() checks version/variant bits, not just the shape).
const VALID_WALLET = "11111111-1111-4111-8111-111111111111";
const MISSING_WALLET = "99999999-9999-4999-8999-999999999999";

function fill(wallet: string, amount: string) {
  fireEvent.change(screen.getByLabelText("Wallet"), { target: { value: wallet } });
  fireEvent.change(screen.getByLabelText("Amount"), { target: { value: amount } });
}

describe("Deposit", () => {
  it("mounts the data door against the real client", () => {
    const { result } = renderHook(() => useDepositModel(), { wrapper });
    expect(result.current.submitting).toBe(false);
    expect(result.current.completed).toBeNull();
  });

  it("blocks an invalid submit with field errors inside the Field anatomy", async () => {
    render(<DepositView />, { wrapper });
    fireEvent.click(screen.getByRole("button", { name: "Deposit" }));
    const alerts = await screen.findAllByRole("alert");
    expect(alerts).toHaveLength(2);
    // Still the form — an invalid submit never reaches the wire.
    expect(screen.getByRole("heading", { name: "Deposit" })).toBeTruthy();
  });

  it("announces while pending, then reaches the success surface", async () => {
    render(<DepositView />, { wrapper });
    fill(VALID_WALLET, "50");
    fireEvent.click(screen.getByRole("button", { name: "Deposit" }));
    await waitFor(() => expect(screen.getByRole("button").getAttribute("aria-busy")).toBe("true"));
    expect(await screen.findByText("Deposit complete")).toBeTruthy();
  });

  it("surfaces the command failure as a role=alert block and keeps the form", async () => {
    render(<DepositView />, { wrapper });
    fill(MISSING_WALLET, "50");
    fireEvent.click(screen.getByRole("button", { name: "Deposit" }));
    const alert = await screen.findByText("We couldn't complete the deposit. Try again.");
    expect(alert.getAttribute("role")).toBe("alert");
    expect(screen.getByLabelText("Wallet")).toBeTruthy();
  });
});
