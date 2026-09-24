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

// Stand-in for the orval hook of the backend's REAL `Deposit` slice (`MapPost("/deposit")` under the `/wallets` group,
// `.WithName(nameof(Deposit))` → operationId `Deposit` → `useDeposit`, the SKY0012 1:1). Like orval's, its variables
// carry the body as `data`.
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
    mutationFn: async ({ data }: { data: DepositInput }): Promise<DepositOutput> => {
      const response = await fetch(`${SAMPLE_API_BASE}/wallets/deposit`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(data),
      });
      if (!response.ok) throw new Error(`deposit failed (${response.status})`);
      return (await response.json()) as DepositOutput;
    },
  });
}

// Stand-in for the orval hook of the backend's `Transfer` slice (`MapPost("/transfer")` under the `/wallets` group,
// `.WithName(nameof(Transfer))` → `useTransfer`). The Idempotency-Key rides as a header, where the slice reads it.
export interface TransferInput {
  fromWalletId: string;
  toWalletId: string;
  amount: number;
}

export interface TransferOutput {
  fromWalletId: string;
  fromBalance: number;
  toWalletId: string;
  toBalance: number;
}

export function useTransfer() {
  return useMutation({
    mutationFn: async ({ data, idempotencyKey }: { data: TransferInput; idempotencyKey: string }): Promise<TransferOutput> => {
      const response = await fetch(`${SAMPLE_API_BASE}/wallets/transfer`, {
        method: "POST",
        headers: { "content-type": "application/json", "idempotency-key": idempotencyKey },
        body: JSON.stringify(data),
      });
      if (!response.ok) throw new Error(`transfer failed (${response.status})`);
      return (await response.json()) as TransferOutput;
    },
  });
}
