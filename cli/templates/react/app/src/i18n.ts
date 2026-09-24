import i18next from "i18next";
import { initReactI18next } from "react-i18next";
import { resources } from "./i18n/resources.generated";

// The app's one i18next instance (`@/i18n`). Each feature owns a `*.i18n.ts` catalog; `skies i18n` assembles them
// into resources.generated.ts, so rerun it after adding or removing a feature.
void i18next.use(initReactI18next).init({
  lng: "en",
  fallbackLng: "en",
  resources,
  interpolation: { escapeValue: false },
});

export default i18next;
