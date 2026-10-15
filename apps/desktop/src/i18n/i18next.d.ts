// Types des clés de traduction : `t("…")` n'accepte que des clés qui existent
// dans les traductions françaises.

import "i18next";
import type { fr } from "./resources";

declare module "i18next" {
  interface CustomTypeOptions {
    defaultNS: "translation";
    resources: {
      translation: typeof fr;
    };
  }
}
