import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "../../App";
import type { ValidationError } from "../../bindings/ValidationError";
import type { Workflow } from "../../bindings/Workflow";
import { Toaster } from "../../components/Toaster";
import { WorkflowsScreen } from "../WorkflowsScreen";

afterEach(() => {
  cleanup();
  clearMocks();
});

const existing: Workflow = {
  schema_version: 1,
  id: "wf_01J8ZK3V9Q4X7M2N5P6R8T0W1Y",
  name: "Devis",
  version: 3,
  enabled: true,
  priority: 10,
  continue_on_match: false,
  trigger: {
    match: "all",
    conditions: [{ type: "file_name_contains", value: "devis", case_sensitive: false }],
    source: "manual",
  },
  actions: [{ type: "copy", destination: "/home/alice/Archives" }],
  owner: { kind: "user", id: "usr_00000000000000000000000000" },
  created_at: "2026-10-30T09:00:00Z",
  updated_at: "2026-10-30T09:00:00Z",
};

/** Backend simulé : `validate` décide des problèmes renvoyés par `validate_workflow`. */
function backend(
  options: { workflows?: Workflow[]; validate?: (draft: unknown) => ValidationError[] } = {},
) {
  const calls: Array<{ command: string; args: Record<string, unknown> }> = [];
  mockIPC(
    (command, payload) => {
      const args = (payload ?? {}) as Record<string, unknown>;
      calls.push({ command, args });
      switch (command) {
        case "storage_status":
          return { available: true, reduced_protection: false, recovered: null };
        case "list_workflows":
          return [...(options.workflows ?? [])];
        case "validate_workflow":
          return options.validate?.(args.draft) ?? [];
        case "preview_name":
          return String(args.template).includes("{")
            ? { kind: "name", name: "2026-03-15_Facture 2026-03-15" }
            : { kind: "name", name: String(args.template) };
        case "create_workflow":
        case "update_workflow":
          return { ...existing, ...(args.draft as object) };
        case "plugin:dialog|open":
          return "C:\\Users\\Alice\\Comptabilité";
        default:
          return undefined;
      }
    },
    { shouldMockEvents: true },
  );
  return calls;
}

function renderScreen() {
  return render(
    <>
      <WorkflowsScreen />
      <Toaster />
    </>,
  );
}

const nbsp = (text: string) => text.replaceAll("\u00a0", " ");

describe("WizardScreen", () => {
  it("creates a workflow step by step", async () => {
    const calls = backend();
    const user = userEvent.setup();
    renderScreen();

    await user.click(await screen.findByRole("button", { name: "Créer un workflow" }));
    expect(document.activeElement?.textContent).toContain("Étape 1 sur 3");
    await user.type(screen.getByLabelText("Texte à trouver dans le nom"), "facture");
    await user.click(screen.getByRole("button", { name: "Suivant" }));

    expect(await screen.findByText(/Étape 2 sur 3/)).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "+ Renommer" }));
    const template = screen.getByLabelText("Nouveau nom");
    await user.clear(template);
    await user.type(template, "{{date_in_name:YYYY-MM-DD}_");
    await user.click(screen.getByRole("button", { name: /Insérer\s?: Nom d'origine/ }));
    expect((template as HTMLInputElement).value).toBe("{date_in_name:YYYY-MM-DD}_{original_name}");
    expect(nbsp((await screen.findByText(/deviendra/)).textContent ?? "")).toContain(
      "« Facture 2026-03-15.pdf » deviendra « 2026-03-15_Facture 2026-03-15 »",
    );
    await user.click(screen.getByRole("button", { name: "+ Déplacer" }));
    await user.click(screen.getByRole("button", { name: "Choisir un dossier…" }));
    expect(await screen.findByText("C:\\Users\\Alice\\Comptabilité")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Suivant" }));

    expect(await screen.findByText(/Étape 3 sur 3/)).toBeTruthy();
    const summary = screen.getByRole("region", { name: "Ce que fera le workflow" });
    expect(nbsp(summary.textContent ?? "")).toContain("Quand Le nom contient « facture »");
    expect(summary.textContent).toContain("puis déplacer vers Comptabilité");
    expect((screen.getByLabelText("Nom du workflow") as HTMLInputElement).value).toBe("Facture");
    await user.click(screen.getByRole("button", { name: "Créer le workflow" }));

    expect(await screen.findByText("Workflow créé.")).toBeTruthy();
    expect(calls.find((call) => call.command === "create_workflow")?.args.draft).toEqual({
      name: "Facture",
      enabled: true,
      continue_on_match: false,
      trigger: {
        match: "all",
        conditions: [{ type: "file_name_contains", value: "facture", case_sensitive: false }],
        source: "manual",
      },
      actions: [
        { type: "rename", template: "{date_in_name:YYYY-MM-DD}_{original_name}" },
        { type: "move", destination: "C:\\Users\\Alice\\Comptabilité", create_folders: false },
      ],
    });
    // Retour à la liste.
    expect(await screen.findByText("Aucun workflow pour l'instant")).toBeTruthy();
  });

  it("goes back without losing anything", async () => {
    backend();
    const user = userEvent.setup();
    renderScreen();

    await user.click(await screen.findByRole("button", { name: "Créer un workflow" }));
    await user.type(screen.getByLabelText("Texte à trouver dans le nom"), "facture");
    await user.click(screen.getByRole("button", { name: "Suivant" }));
    await screen.findByText(/Étape 2 sur 3/);
    await user.click(screen.getByRole("button", { name: "Précédent" }));

    expect((screen.getByLabelText("Texte à trouver dans le nom") as HTMLInputElement).value).toBe(
      "facture",
    );
  });

  it("shows a validation problem on its field and stays on the step", async () => {
    backend({ validate: () => [{ path: "trigger.conditions[0]", code: "empty_search_text" }] });
    const user = userEvent.setup();
    renderScreen();

    await user.click(await screen.findByRole("button", { name: "Créer un workflow" }));
    await user.click(screen.getByRole("button", { name: "Suivant" }));

    const field = screen.getByLabelText("Texte à trouver dans le nom");
    await waitFor(() => expect(field.getAttribute("aria-invalid")).toBe("true"));
    expect(screen.getByText("Indiquez le texte à trouver.")).toBeTruthy();
    expect(screen.getByText(/Étape 1 sur 3/)).toBeTruthy();
  });

  it("refuses a malformed name template before validating", async () => {
    const calls = backend();
    const user = userEvent.setup();
    mockIPC(
      (command, payload) => {
        const args = (payload ?? {}) as Record<string, unknown>;
        calls.push({ command, args });
        if (command === "preview_name") {
          return String(args.template).includes("{") && !String(args.template).includes("}")
            ? { kind: "invalid_template" }
            : { kind: "name", name: "ok" };
        }
        return command === "list_workflows" || command === "validate_workflow" ? [] : undefined;
      },
      { shouldMockEvents: true },
    );
    renderScreen();

    await user.click(await screen.findByRole("button", { name: "Créer un workflow" }));
    await user.type(screen.getByLabelText("Texte à trouver dans le nom"), "facture");
    await user.click(screen.getByRole("button", { name: "Suivant" }));
    await screen.findByText(/Étape 2 sur 3/);
    await user.click(screen.getByRole("button", { name: "+ Renommer" }));
    const template = screen.getByLabelText("Nouveau nom");
    await user.clear(template);
    await user.type(template, "{{date");
    await user.click(screen.getByRole("button", { name: "Suivant" }));

    expect(await screen.findByText(/Le modèle de nom est mal écrit/)).toBeTruthy();
    expect(screen.getByText(/Étape 2 sur 3/)).toBeTruthy();
  });

  it("edits an existing workflow at its version", async () => {
    const calls = backend({ workflows: [existing] });
    const user = userEvent.setup();
    renderScreen();

    await user.click(await screen.findByRole("button", { name: /Modifier «\sDevis\s»/ }));
    expect(screen.getByText(/Modifier «\sDevis\s»/)).toBeTruthy();
    expect((screen.getByLabelText("Texte à trouver dans le nom") as HTMLInputElement).value).toBe(
      "devis",
    );
    await user.click(screen.getByRole("button", { name: "Suivant" }));
    await screen.findByText(/Étape 2 sur 3/);
    expect(screen.getByText("/home/alice/Archives")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Suivant" }));
    await screen.findByText(/Étape 3 sur 3/);
    const name = screen.getByLabelText("Nom du workflow");
    await user.clear(name);
    await user.type(name, "Devis clients");
    await user.click(screen.getByRole("button", { name: "Enregistrer" }));

    expect(await screen.findByText("Modifications enregistrées.")).toBeTruthy();
    const update = calls.find((call) => call.command === "update_workflow");
    expect(update?.args).toMatchObject({
      id: existing.id,
      expectedVersion: 3,
      draft: { name: "Devis clients" },
    });
  });

  it("keeps the draft when switching sections", async () => {
    backend();
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "Créer un workflow" }));
    await user.type(screen.getByLabelText("Texte à trouver dans le nom"), "facture");
    const navigation = screen.getByRole("navigation");
    await user.click(within(navigation).getByRole("button", { name: "Historique" }));
    await user.click(within(navigation).getByRole("button", { name: "Workflows" }));

    expect((screen.getByLabelText("Texte à trouver dans le nom") as HTMLInputElement).value).toBe(
      "facture",
    );
  });

  it("asks before discarding a started workflow", async () => {
    backend();
    const user = userEvent.setup();
    renderScreen();

    await user.click(await screen.findByRole("button", { name: "Créer un workflow" }));
    await user.type(screen.getByLabelText("Texte à trouver dans le nom"), "f");
    await user.click(screen.getByRole("button", { name: "Annuler" }));
    const dialog = screen.getByRole("alertdialog");
    await user.click(within(dialog).getByRole("button", { name: "Annuler" }));
    expect(screen.getByLabelText("Texte à trouver dans le nom")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Annuler" }));
    await user.click(
      within(screen.getByRole("alertdialog")).getByRole("button", { name: "Abandonner" }),
    );
    expect(await screen.findByText("Aucun workflow pour l'instant")).toBeTruthy();
  });
});
