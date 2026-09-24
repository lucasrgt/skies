// Feature-scoped copy for the Deposit form recipe. Three locales with identical keys (SKYFE011).
export const ptBR = {
  title: "Depositar",
  submit: "Depositar",
  "errors.walletId": "Informe um id de carteira válido.",
  "errors.amount": "O valor precisa ser maior que zero.",
  "errors.submit": "Não foi possível concluir o depósito. Tente novamente.",
  "fields.walletId.label": "Carteira",
  "fields.walletId.hint": "O identificador (UUID) da carteira de destino.",
  "fields.walletId.placeholder": "00000000-0000-0000-0000-000000000001",
  "fields.amount.label": "Valor",
  "done.title": "Depósito concluído",
  "done.description": "O saldo da carteira foi atualizado.",
} as const;

export const esES = {
  title: "Depositar",
  submit: "Depositar",
  "errors.walletId": "Introduce un id de cartera válido.",
  "errors.amount": "El importe debe ser mayor que cero.",
  "errors.submit": "No pudimos completar el depósito. Inténtalo de nuevo.",
  "fields.walletId.label": "Cartera",
  "fields.walletId.hint": "El identificador (UUID) de la cartera de destino.",
  "fields.walletId.placeholder": "00000000-0000-0000-0000-000000000001",
  "fields.amount.label": "Importe",
  "done.title": "Depósito completado",
  "done.description": "El saldo de la cartera se ha actualizado.",
} as const;

export const enUS = {
  title: "Deposit",
  submit: "Deposit",
  "errors.walletId": "Enter a valid wallet id.",
  "errors.amount": "The amount must be greater than zero.",
  "errors.submit": "We couldn't complete the deposit. Try again.",
  "fields.walletId.label": "Wallet",
  "fields.walletId.hint": "The destination wallet's identifier (UUID).",
  "fields.walletId.placeholder": "00000000-0000-0000-0000-000000000001",
  "fields.amount.label": "Amount",
  "done.title": "Deposit complete",
  "done.description": "The wallet balance has been updated.",
} as const;
