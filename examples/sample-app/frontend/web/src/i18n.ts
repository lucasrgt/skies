import i18next from "i18next";
import { initReactI18next } from "react-i18next";
import { resources } from "./i18n/resources.generated";

// Harness i18n instance (`@/i18n`) wired with every feature's catalog, so the ViewModels' `i18n.t("items:error")` /
// `i18n.t("transfer:errors.submit")` resolve. The resource tree is assembled from each feature's `*.i18n.ts` by
// `skies i18n`; rerun it after adding or removing a feature.
void i18next.use(initReactI18next).init({
  lng: "en",
  fallbackLng: "en",
  resources,
  interpolation: { escapeValue: false },
});

export default i18next;
