// Feature-scoped copy for the Transfer form. Three locales with identical keys (SKYFE011).
export const ptBR = {
  title: "Transferir",
  submit: "Transferir",
  "errors.fromWalletId": "Informe um id de carteira de origem válido.",
  "errors.toWalletId": "Informe um id de carteira de destino válido.",
  "errors.sameWallet": "Escolha uma carteira diferente para enviar.",
  "errors.amount": "O valor precisa ser maior que zero.",
  "errors.submit": "Não foi possível concluir a transferência. Confira o saldo e tente novamente.",
  "fields.fromWalletId.label": "Carteira de origem",
  "fields.toWalletId.label": "Carteira de destino",
  "fields.walletId.placeholder": "00000000-0000-0000-0000-000000000001",
  "fields.amount.label": "Valor",
  "done.title": "Transferência concluída",
  "done.description": "Os saldos das duas carteiras foram atualizados.",
} as const;

export const esES = {
  title: "Transferir",
  submit: "Transferir",
  "errors.fromWalletId": "Introduce un id de cartera de origen válido.",
  "errors.toWalletId": "Introduce un id de cartera de destino válido.",
  "errors.sameWallet": "Elige una cartera diferente a la que enviar.",
  "errors.amount": "El importe debe ser mayor que cero.",
  "errors.submit": "No pudimos completar la transferencia. Revisa el saldo e inténtalo de nuevo.",
  "fields.fromWalletId.label": "Cartera de origen",
  "fields.toWalletId.label": "Cartera de destino",
  "fields.walletId.placeholder": "00000000-0000-0000-0000-000000000001",
  "fields.amount.label": "Importe",
  "done.title": "Transferencia completada",
  "done.description": "Los saldos de las dos carteras se han actualizado.",
} as const;

export const enUS = {
  title: "Transfer",
  submit: "Transfer",
  "errors.fromWalletId": "Enter a valid source wallet id.",
  "errors.toWalletId": "Enter a valid destination wallet id.",
  "errors.sameWallet": "Choose a different wallet to send to.",
  "errors.amount": "The amount must be greater than zero.",
  "errors.submit": "We couldn't complete the transfer. Check the balance and try again.",
  "fields.fromWalletId.label": "From wallet",
  "fields.toWalletId.label": "To wallet",
  "fields.walletId.placeholder": "00000000-0000-0000-0000-000000000001",
  "fields.amount.label": "Amount",
  "done.title": "Transfer complete",
  "done.description": "Both wallet balances have been updated.",
} as const;
