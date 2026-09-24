import { useMutation, useQuery } from "@tanstack/react-query";
import type { Item } from "../items/Items.viewModel";
import { SAMPLE_API_BASE } from "../api";

// Stand-in for the orval-generated typed hook (`@/client.gen/sample`). It uses the real fetch seam so the
// tests force responses at the HTTP boundary (MSW) instead of accepting an in-memory green.
export function useListItems() {
  return useQuery({
    queryKey: ["sample", "list_items"],
    queryFn: async (): Promise<{ items: Item[] }> => {
      const response = await fetch(`${SAMPLE_API_BASE}/items`);
      if (!response.ok) throw new Error(`list items failed (${response.status})`);
      return (await response.json()) as { items: Item[] };
    },
  });
}

// Stand-in for the orval hook of the backend's REAL `Deposit` slice (`MapPost("/deposit").WithName(nameof(Deposit))`
// → operationId `Deposit` → `useDeposit`, the SKY0012 1:1).
export interface DepositInput {
  walletId: string;
  amount: number;
}

export interface DepositOutput {
  walletId: string;
  balance: number;
}

export function useDeposit() {
  return useMutation({
    mutationFn: async (input: DepositInput): Promise<DepositOutput> => {
      const response = await fetch(`${SAMPLE_API_BASE}/deposit`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(input),
      });
      if (!response.ok) throw new Error(`deposit failed (${response.status})`);
      return (await response.json()) as DepositOutput;
    },
  });
}
