import { useForm, type Control } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { submitOrReveal } from "@skiesjs/react";
import { use{{ name }} } from "@/client.gen/{{ client }}";
import i18n from "@/i18n";

// The form lives in the ViewModel so a spec drives its rules through this hook. Every input holds a string; the
// schema restates the slice's own rules (from its contract) and the submit converts at the boundary. The backend
// stays the authority.

export interface {{ name }}Form {
{%- for field in fields %}
  {{ field.name }}: string;
{%- endfor %}
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
{%- for field in fields %}
    {{ field.name }}: {{ field.rule }},
{%- endfor %}
  });

  const form = useForm<{{ name }}Form>({
    resolver: zodResolver(schema),
    defaultValues: { {% for field in fields %}{{ field.name }}: ""{% if not loop.last %}, {% endif %}{% endfor %} },
  });

  // An invalid submit focuses the first invalid field instead of doing nothing.
  const submit = submitOrReveal(
    form.handleSubmit,
    (values) => mutation.mutate({{ variables }}),
    { onInvalid: (first) => form.setFocus(first), order: [{% for field in fields %}"{{ field.name }}"{% if not loop.last %}, {% endif %}{% endfor %}] },
  );

  return {
    control: form.control,
    submit: () => void submit(),
    submitting: mutation.isPending,
    submitError: mutation.isError ? i18n.t("{{ lower }}:errors.submit") : null,
    completed: mutation.isSuccess,
  };
}
