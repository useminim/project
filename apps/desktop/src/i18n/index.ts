// Initialisation des traductions. Toutes les clés sont dans un seul espace de
// noms, organisé par section : `t("workflows.title")`, `t("errors.not_found")`.
// Les clés d'erreur reprennent le `message_key` des erreurs Rust.

import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import { en, fr } from "./resources";

export const defaultLanguage = "fr";

void i18n.use(initReactI18next).init({
  lng: defaultLanguage,
  fallbackLng: defaultLanguage,
  resources: {
    fr: { translation: fr },
    en: { translation: en },
  },
  interpolation: {
    // React échappe déjà le texte affiché.
    escapeValue: false,
  },
});

export { i18n };
