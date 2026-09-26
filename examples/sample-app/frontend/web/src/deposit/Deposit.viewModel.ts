import { useForm, type Control } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { submitOrReveal } from "@skiesjs/react";
import { useDeposit, type DepositOutput } from "@/client.gen/sample";
import i18n from "@/i18n";

// CANONICAL FORM UNIT — the form recipe's data door. The `useForm` lives HERE (form logic, not rendering —
// FRONTEND-CONVENTIONS.md §Forms), so the View only binds `control` and the submit, and the form's rules are tested
// through the ViewModel. The zod schema is contract-grounded: it restates ONLY the Deposit slice's validation surface
// (walletId required+uuid, amount > 0 — Money's rule) and never invents rules the backend doesn't hold.

export interface DepositForm {
  walletId: string;
  // The control hands the View a string; the wire wants a number — coerced once, at the submit boundary.
  amount: string;
}

export interface DepositModel {
  control: Control<DepositForm>;
  submit: () => void;
  submitting: boolean;
  /** The command's failure surface (the SKYFE013 discipline): the mutation's error state, localized. */
  submitError: string | null;
  /** The command's success surface — a routed app redirects on it (declarative); the sample renders done. */
  completed: DepositOutput | null;
}

export function useDepositModel(): DepositModel {
  const mutation = useDeposit();

  const schema = z.object({
    walletId: z.uuid(i18n.t("deposit:errors.walletId")),
    amount: z.string().refine((v) => Number(v) > 0, i18n.t("deposit:errors.amount")),
  });

  const form = useForm<DepositForm>({
    resolver: zodResolver(schema),
    defaultValues: { walletId: "", amount: "" },
  });

  // The submit always carries its invalid path (SKYFE031): submitOrReveal forces the surface and resolves the
  // first invalid field. On this single-screen form the reveal is a focus — the inline field errors (SKYFE032's
  // fieldState surface in the View) do the showing; a multi-tab shell would map the field to its tab instead.
  const submit = submitOrReveal(
    form.handleSubmit,
    (values) => mutation.mutate({ data: { walletId: values.walletId, amount: Number(values.amount) } }),
    { onInvalid: (first) => form.setFocus(first), order: ["walletId", "amount"] },
  );

  return {
    control: form.control,
    submit: () => void submit(),
    submitting: mutation.isPending,
    submitError: mutation.isError ? i18n.t("deposit:errors.submit") : null,
    completed: mutation.isSuccess ? mutation.data : null,
  };
}
