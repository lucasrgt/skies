// Copy for the {{ plural }} screen, one export per app locale with the same keys. Translate each locale's strings.
{% for locale in locales %}{% if not loop.first %}
{% endif %}export const {{ locale }} = {
  error: "We couldn't load.",
  "empty.title": "Nothing here yet",
  "empty.description": "What you create will show up here.",
} as const;
{% endfor %}