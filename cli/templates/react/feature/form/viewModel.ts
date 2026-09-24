import { useForm, type Control } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { submitOrReveal } from "@skiesjs/react";
// The orval-generated mutation hook of the `{{ name }}` slice (`.WithName(nameof({{ name }}))`) — the ONLY data the
// door touches.
import { use{{ name }} } from "@/client.gen/{{ client }}";
import i18n from "@/i18n";

// FORM UNIT — the ViewModel of a command screen. The `useForm` lives here (form logic, not rendering), so the View
// only binds `control` and the submit and a spec case drives the rules through this hook. The zod schema restates
// ONLY the slice's own validation surface; it never invents a rule the backend does not hold. The fields start as
// the `g slice` scaffold's Input (`Id`): replace them with the slice's real Input.

export interface {{ name }}Form {
  // The control hands the View strings; convert at the submit boundary (e.g. `Number(values.amount)`).
  id: string;
}

export interface {{ name }}Model {
  control: Control<{{ name }}Form>;
  submit: () => void;
  submitting: boolean;
  /** The command's failure surface: the mutation's error state, localized. */
  submitError: string | null;
  /** The command's success surface: a routed app redirects on it declaratively. */
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

  // The submit always carries its invalid path: submitOrReveal forces the surface and resolves the first
  // invalid field, focused here; the inline field errors in the View do the showing.
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
