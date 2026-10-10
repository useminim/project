// Préparation des tests d'interface (environnement happy-dom).

import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";
import { i18n } from "../i18n";

afterEach(async () => {
  cleanup();
  await i18n.changeLanguage("fr");
});
