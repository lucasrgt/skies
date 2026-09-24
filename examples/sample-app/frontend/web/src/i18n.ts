import i18next from "i18next";
import { initReactI18next } from "react-i18next";
import { enUS as depositEnUS, esES as depositEsES, ptBR as depositPtBR } from "./deposit/deposit.i18n";
import { enUS, esES, ptBR } from "./items/items.i18n";

// Harness i18n instance (`@/i18n`) wired with the sample features' catalogs, so the ViewModels'
// `i18n.t("items:error")` / `i18n.t("deposit:errors.submit")` resolve. A real Skies Framework app assembles this
// resource tree from every feature's `*.i18n.ts` (the generator the roadmap calls "i18n assembly"); here we wire
// the two sample features by hand.
void i18next.use(initReactI18next).init({
  lng: "en",
  fallbackLng: "en",
  resources: {
    en: { items: enUS, deposit: depositEnUS },
    pt: { items: ptBR, deposit: depositPtBR },
    es: { items: esES, deposit: depositEsES },
  },
  interpolation: { escapeValue: false },
});

export default i18next;
