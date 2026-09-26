import { useState } from "react";
import { useForm, type Control } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { submitOrReveal } from "@skiesjs/react";
import { useTransfer, type TransferOutput } from "@/client.gen/sample";
import i18n from "@/i18n";

// The Transfer form's data door (the Deposit recipe's shape). The zod schema restates only the Transfer slice's
// validation surface — both ids required uuids, amount > 0, and the two wallets different (the slice's 422, caught
// before the wire) — and invents no rule the backend doesn't hold.

export interface TransferForm {
  fromWalletId: string;
  toWalletId: string;
  // The control hands the View a string; the wire wants a number — coerced once, at the submit boundary.
  amount: string;
}

export interface TransferModel {
  control: Control<TransferForm>;
  submit: () => void;
  submitting: boolean;
  /** The command's failure surface (SKYFE013): the mutation's error state, localized. */
  submitError: string | null;
  /** The command's success surface — a routed app redirects on it (declarative); the sample renders done. */
  completed: TransferOutput | null;
}

export function useTransferModel(): TransferModel {
  const mutation = useTransfer();
  // One key per screen: a resubmit after a network failure replays instead of moving the money twice.
  const [idempotencyKey] = useState(() => crypto.randomUUID());

  const schema = z
    .object({
      fromWalletId: z.uuid(i18n.t("transfer:errors.fromWalletId")),
      toWalletId: z.uuid(i18n.t("transfer:errors.toWalletId")),
      amount: z.string().refine((v) => Number(v) > 0, i18n.t("transfer:errors.amount")),
    })
    .refine((v) => v.fromWalletId !== v.toWalletId, {
      path: ["toWalletId"],
      message: i18n.t("transfer:errors.sameWallet"),
    });

  const form = useForm<TransferForm>({
    resolver: zodResolver(schema),
    defaultValues: { fromWalletId: "", toWalletId: "", amount: "" },
  });

  const submit = submitOrReveal(
    form.handleSubmit,
    (values) =>
      mutation.mutate({
        data: { fromWalletId: values.fromWalletId, toWalletId: values.toWalletId, amount: Number(values.amount) },
        idempotencyKey,
      }),
    { onInvalid: (first) => form.setFocus(first), order: ["fromWalletId", "toWalletId", "amount"] },
  );

  return {
    control: form.control,
    submit: () => void submit(),
    submitting: mutation.isPending,
    submitError: mutation.isError ? i18n.t("transfer:errors.submit") : null,
    completed: mutation.isSuccess ? mutation.data : null,
  };
}
