import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import type { Workflow } from "../bindings/Workflow";
import { Toaster } from "../components/Toaster";
import { WorkflowsScreen } from "./WorkflowsScreen";

afterEach(() => {
  // Démonte l'écran (désabonnement de l'événement) avant de retirer la simulation IPC.
  cleanup();
  clearMocks();
});

function workflow(n: number, name: string, enabled = true): Workflow {
  return {
    schema_version: 1,
    id: `wf_01J8ZK3V9Q4X7M2N5P6R8T0W1${n}`,
    name,
    version: 1,
    enabled,
    priority: n * 10,
    continue_on_match: false,
    trigger: {
      match: "all",
      conditions: [
        { type: "file_name_contains", value: name.toLowerCase(), case_sensitive: false },
      ],
      source: "manual",
    },
    actions: [{ type: "move", destination: `/home/alice/${name}`, create_folders: false }],
    owner: { kind: "user", id: "usr_00000000000000000000000000" },
    created_at: "2026-10-30T09:00:00Z",
    updated_at: "2026-10-30T09:00:00Z",
  };
}

/** Backend simulé : garde la liste et enregistre les commandes reçues. */
function backend(initial: Workflow[]) {
  let workflows = [...initial];
  const calls: Array<{ command: string; args: Record<string, unknown> }> = [];
  mockIPC(
    (command, payload) => {
      const args = (payload ?? {}) as Record<string, unknown>;
      calls.push({ command, args });
      switch (command) {
        case "list_workflows":
          // Copie, comme une vraie réponse IPC.
          return [...workflows];
        case "set_workflow_enabled": {
          workflows = workflows.map((w) =>
            w.id === args.id
              ? { ...w, enabled: args.enabled as boolean, version: w.version + 1 }
              : w,
          );
          return workflows.find((w) => w.id === args.id);
        }
        case "delete_workflow":
          workflows = workflows.filter((w) => w.id !== args.id);
          return null;
        case "reorder_workflows": {
          const order = args.ordered as string[];
          workflows = order.map((id, i) => ({
            ...(workflows.find((w) => w.id === id) as Workflow),
            priority: (i + 1) * 10,
          }));
          return workflows;
        }
        default:
          return undefined;
      }
    },
    { shouldMockEvents: true },
  );
  return { calls, current: () => workflows };
}

function names() {
  return within(screen.getByRole("list", { name: /Liste des workflows/ }))
    .getAllByRole("heading")
    .map((heading) => heading.textContent);
}

function renderScreen() {
  return render(
    <>
      <WorkflowsScreen />
      <Toaster />
    </>,
  );
}

describe("WorkflowsScreen", () => {
  it("lists workflows in order with a plain-language summary", async () => {
    backend([workflow(1, "Factures"), workflow(2, "Devis", false)]);
    renderScreen();

    expect(await screen.findByRole("heading", { name: "Factures" })).toBeTruthy();
    expect(names()).toEqual(["Factures", "Devis"]);
    expect(screen.getByText(/essayés de haut en bas/)).toBeTruthy();
    const first = screen.getAllByRole("listitem")[0] as HTMLElement;
    expect(first.textContent?.replaceAll("\u00a0", " ")).toContain(
      "Quand Le nom contient « factures »",
    );
    expect(first.textContent).toContain("Alors Déplacer vers Factures");
    expect(
      screen.getByRole("switch", { name: /Activer «\sDevis\s»/ }).getAttribute("aria-checked"),
    ).toBe("false");
  });

  it("invites to create a first workflow when the list is empty", async () => {
    backend([]);
    renderScreen();

    expect(await screen.findByText("Aucun workflow pour l'instant")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Créer un workflow" })).toBeTruthy();
  });

  it("toggles a workflow", async () => {
    const { calls } = backend([workflow(1, "Factures")]);
    const user = userEvent.setup();
    renderScreen();

    const toggle = await screen.findByRole("switch", { name: /Activer «\sFactures\s»/ });
    await user.click(toggle);

    await waitFor(() => expect(toggle.getAttribute("aria-checked")).toBe("false"));
    expect(calls).toContainEqual({
      command: "set_workflow_enabled",
      args: { id: workflow(1, "").id, enabled: false },
    });
  });

  it("deletes only after confirmation", async () => {
    const { calls } = backend([workflow(1, "Factures"), workflow(2, "Devis")]);
    const user = userEvent.setup();
    renderScreen();

    await user.click(await screen.findByRole("button", { name: /Supprimer «\sDevis\s»/ }));
    await user.click(
      within(screen.getByRole("alertdialog")).getByRole("button", { name: "Annuler" }),
    );
    expect(calls.some((call) => call.command === "delete_workflow")).toBe(false);
    expect(names()).toEqual(["Factures", "Devis"]);

    await user.click(screen.getByRole("button", { name: /Supprimer «\sDevis\s»/ }));
    await user.click(
      within(screen.getByRole("alertdialog")).getByRole("button", { name: "Supprimer" }),
    );

    await waitFor(() => expect(names()).toEqual(["Factures"]));
    expect(await screen.findByText("Workflow supprimé.")).toBeTruthy();
  });

  it("reorders with the move buttons", async () => {
    const { calls } = backend([workflow(1, "A"), workflow(2, "B"), workflow(3, "C")]);
    const user = userEvent.setup();
    renderScreen();

    await user.click(await screen.findByRole("button", { name: /Descendre «\sA\s»/ }));

    await waitFor(() => expect(names()).toEqual(["B", "A", "C"]));
    expect(calls).toContainEqual({
      command: "reorder_workflows",
      args: { ordered: [workflow(2, "").id, workflow(1, "").id, workflow(3, "").id] },
    });
    expect(
      (screen.getByRole("button", { name: /Monter «\sB\s»/ }) as HTMLButtonElement).disabled,
    ).toBe(true);
  });

  it("reorders with the keyboard on the drag handle", async () => {
    backend([workflow(1, "A"), workflow(2, "B"), workflow(3, "C")]);
    const user = userEvent.setup();
    renderScreen();

    const handle = await screen.findByRole("button", { name: /Déplacer «\sC\s»/ });
    handle.focus();
    await user.keyboard(" ");
    await user.keyboard("{ArrowUp}");
    await user.keyboard(" ");

    await waitFor(() => expect(names()).not.toEqual(["A", "B", "C"]));
  });

  it("reloads after a change made elsewhere", async () => {
    const { current } = backend([workflow(1, "Factures")]);
    renderScreen();
    await screen.findByRole("heading", { name: "Factures" });

    current().push(workflow(2, "Devis"));
    await act(() => emit("workflows_changed"));

    expect(await screen.findByRole("heading", { name: "Devis" })).toBeTruthy();
  });

  it("explains a version conflict and reloads", async () => {
    let listed = 0;
    mockIPC(
      (command) => {
        if (command === "list_workflows") {
          listed += 1;
          return [workflow(1, "Factures")];
        }
        if (command === "set_workflow_enabled") {
          throw { code: "conflict", message_key: "errors.conflict" };
        }
        return undefined;
      },
      { shouldMockEvents: true },
    );
    const user = userEvent.setup();
    renderScreen();

    await user.click(await screen.findByRole("switch", { name: /Activer «\sFactures\s»/ }));

    expect(
      await screen.findByText(/modifié entre-temps\s:\sla liste a été rechargée/),
    ).toBeTruthy();
    await waitFor(() => expect(listed).toBe(2));
  });
});
