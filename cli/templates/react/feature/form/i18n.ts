// Feature-scoped copy for a command screen. Three locales with identical keys (SKYFE011) — fill in the real strings.
export const ptBR = {
  title: "{{ name }}",
  submit: "{{ name }}",
  "errors.id": "Informe um id válido.",
  "errors.submit": "Não foi possível concluir. Tente novamente.",
  "fields.id.label": "Id",
  "fields.id.hint": "O identificador (UUID).",
  "done.title": "Concluído",
  "done.description": "A operação foi concluída.",
} as const;

export const esES = {
  title: "{{ name }}",
  submit: "{{ name }}",
  "errors.id": "Introduce un id válido.",
  "errors.submit": "No pudimos completarlo. Inténtalo de nuevo.",
  "fields.id.label": "Id",
  "fields.id.hint": "El identificador (UUID).",
  "done.title": "Completado",
  "done.description": "La operación se ha completado.",
} as const;

export const enUS = {
  title: "{{ name }}",
  submit: "{{ name }}",
  "errors.id": "Enter a valid id.",
  "errors.submit": "We couldn't complete it. Try again.",
  "fields.id.label": "Id",
  "fields.id.hint": "The identifier (UUID).",
  "done.title": "Done",
  "done.description": "The operation is complete.",
} as const;
