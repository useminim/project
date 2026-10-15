// Préparation des tests d'interface (environnement happy-dom).

import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";
import { i18n } from "../i18n";
import { useUiStore } from "../state/ui";

const initialUiState = useUiStore.getState();

afterEach(async () => {
  cleanup();
  useUiStore.setState(initialUiState, true);
  await i18n.changeLanguage("fr");
});
