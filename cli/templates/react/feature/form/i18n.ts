// Copy for the {{ name }} screen, one export per app locale with the same keys. Translate each locale's strings.
{% for locale in locales %}{% if not loop.first %}
{% endif %}export const {{ locale }} = {
  title: "{{ title }}",
  submit: "{{ title }}",
  "errors.submit": "We couldn't complete it. Try again.",
{%- for field in fields %}
  "errors.{{ field.name }}": "{{ field.error }}",
{%- endfor %}
{%- for field in fields %}
  "fields.{{ field.name }}.label": "{{ field.label }}",
{%- endfor %}
  "done.title": "Done",
  "done.description": "The operation is complete.",
} as const;
{% endfor %}