// Traductions, un fichier par section. Le français fait foi : les autres
// langues doivent avoir exactement les mêmes clés, ce que `tsc` vérifie.

import enCommon from "./en/common.json";
import enErrors from "./en/errors.json";
import enHistory from "./en/history.json";
import enNavigation from "./en/navigation.json";
import enSettings from "./en/settings.json";
import enStorage from "./en/storage.json";
import enWaitingZone from "./en/waitingZone.json";
import enWorkflows from "./en/workflows.json";
import frCommon from "./fr/common.json";
import frErrors from "./fr/errors.json";
import frHistory from "./fr/history.json";
import frNavigation from "./fr/navigation.json";
import frSettings from "./fr/settings.json";
import frStorage from "./fr/storage.json";
import frWaitingZone from "./fr/waitingZone.json";
import frWorkflows from "./fr/workflows.json";

export const fr = {
  common: frCommon,
  errors: frErrors,
  history: frHistory,
  navigation: frNavigation,
  settings: frSettings,
  storage: frStorage,
  waitingZone: frWaitingZone,
  workflows: frWorkflows,
};

/** Même structure que `T`, avec un texte à chaque feuille. */
type SameKeys<T> = { [K in keyof T]: T[K] extends string ? string : SameKeys<T[K]> };

export const en: SameKeys<typeof fr> = {
  common: enCommon,
  errors: enErrors,
  history: enHistory,
  navigation: enNavigation,
  settings: enSettings,
  storage: enStorage,
  waitingZone: enWaitingZone,
  workflows: enWorkflows,
};

export const languages = ["fr", "en"] as const;

export type Language = (typeof languages)[number];
