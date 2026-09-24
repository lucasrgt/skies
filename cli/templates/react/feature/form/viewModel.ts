import { useForm, type Control } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { submitOrReveal } from "@skiesjs/react";
import { use{{ name }} } from "@/client.gen/{{ client }}";
import i18n from "@/i18n";

// The form lives in the ViewModel so a spec drives its rules through this hook. The schema restates only the
// slice's own validation; the backend stays the authority.

export interface {{ name }}Form {
  // Placeholder: mirrors the `g slice` scaffold's Input(Guid Id). Replace it with the slice's real fields.
  id: string;
}

export interface {{ name }}Model {
  control: Control<{{ name }}Form>;
  submit: () => void;
  submitting: boolean;
  submitError: string | null;
  completed: boolean;
}

export function use{{ name }}Model(): {{ name }}Model {
  const mutation = use{{ name }}();

  const schema = z.object({
    id: z.uuid(i18n.t("{{ lower }}:errors.id")),
  });

  const form = useForm<{{ name }}Form>({
    resolver: zodResolver(schema),
    defaultValues: { id: "" },
  });

  // An invalid submit focuses the first invalid field instead of doing nothing.
  const submit = submitOrReveal(
    form.handleSubmit,
    (values) => mutation.mutate({ data: { id: values.id } }),
    { onInvalid: (first) => form.setFocus(first), order: ["id"] },
  );

  return {
    control: form.control,
    submit: () => void submit(),
    submitting: mutation.isPending,
    submitError: mutation.isError ? i18n.t("{{ lower }}:errors.submit") : null,
    completed: mutation.isSuccess,
  };
}
