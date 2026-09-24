// Copy for the {{ name }} screen, one export per app locale with the same keys. Translate each locale's strings.
{% for locale in locales %}{% if not loop.first %}
{% endif %}export const {{ locale }} = {
  title: "{{ name }}",
  submit: "{{ name }}",
  "errors.id": "Enter a valid id.",
  "errors.submit": "We couldn't complete it. Try again.",
  "fields.id.label": "Id",
  "fields.id.hint": "The identifier (UUID).",
  "done.title": "Done",
  "done.description": "The operation is complete.",
} as const;
{% endfor %}