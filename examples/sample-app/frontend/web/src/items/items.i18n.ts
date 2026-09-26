// Feature-scoped copy for the Items sample. Three locales with identical keys — the i18n-completeness a frontend
// doctor rule will enforce (every key present in every locale, no hardcoded strings in the View/ViewModel).
export const ptBR = {
  title: "Itens",
  loading: "Carregando…",
  retry: "Tentar novamente",
  error: "Não foi possível carregar os itens.",
  "empty.title": "Nada por aqui ainda",
  "empty.description": "Os itens que você criar aparecem aqui.",
} as const;

export const esES = {
  title: "Elementos",
  loading: "Cargando…",
  retry: "Reintentar",
  error: "No pudimos cargar los elementos.",
  "empty.title": "Nada por aquí todavía",
  "empty.description": "Los elementos que crees aparecerán aquí.",
} as const;

export const enUS = {
  title: "Items",
  loading: "Loading…",
  retry: "Try again",
  error: "We couldn't load the items.",
  "empty.title": "Nothing here yet",
  "empty.description": "Items you create will show up here.",
} as const;
