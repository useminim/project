// Préparation des tests d'interface (environnement happy-dom).

import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";
import { i18n } from "../i18n";
import { useUiStore } from "../state/ui";
import { useWizardStore } from "../state/wizard";

const initialUiState = useUiStore.getState();
const initialWizardState = useWizardStore.getState();

afterEach(async () => {
  cleanup();
  localStorage.clear();
  useUiStore.setState(initialUiState, true);
  useWizardStore.setState(initialWizardState, true);
  await i18n.changeLanguage("fr");
});
