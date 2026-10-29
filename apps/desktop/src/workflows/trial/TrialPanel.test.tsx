import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import type { TrialOutcome } from "../../bindings/TrialOutcome";
import type { WorkflowDraft } from "../../bindings/WorkflowDraft";
import { TrialPanel } from "./TrialPanel";

afterEach(() => {
  cleanup();
  clearMocks();
});

const FILE = "C:\\Users\\Alice\\Téléchargements\\Facture 15.03.2026.pdf";

const draft: WorkflowDraft = {
  name: "Factures",
  enabled: true,
  continue_on_match: false,
  trigger: {
    match: "all",
    conditions: [
      { type: "file_name_contains", value: "facture", case_sensitive: false },
      { type: "file_name_matches_date" },
      { type: "file_extension_is", values: ["pdf"] },
    ],
    source: "manual",
  },
  actions: [
    { type: "rename", template: "{date_in_name:YYYY-MM-DD}_{original_name}.{extension}" },
    { type: "move", destination: "C:\\Users\\Alice\\Comptabilité", create_folders: false },
  ],
};

const renamed = "C:\\Users\\Alice\\Téléchargements\\2026-03-15_Facture 15.03.2026.pdf";
const moved = "C:\\Users\\Alice\\Comptabilité\\2026-03-15_Facture 15.03.2026.pdf";
const fingerprint = "ab".repeat(32);

function outcome(executed: boolean, overrides: Partial<TrialOutcome> = {}): TrialOutcome {
  return {
    evaluation: {
      result: "match",
      conditions: [
        { path: "trigger.conditions[0]", result: "match", explanation: "text_search" },
        {
          path: "trigger.conditions[1]",
          result: "match",
          explanation: "date_found",
          date: "2026-03-15",
        },
        { path: "trigger.conditions[2]", result: "match", explanation: "extension", found: "pdf" },
      ],
    },
    executed,
    actions: [
      {
        type: "rename",
        status: "success",
        duration_ms: 3,
        operations: [{ kind: "rename", from: FILE, to: renamed, fingerprint }],
      },
      {
        type: "move",
        status: "success",
        duration_ms: 4,
        operations: [{ kind: "move", from: renamed, to: moved, fingerprint }],
      },
    ],
    location: moved,
    ...overrides,
  };
}

function backend(preview: TrialOutcome, run: TrialOutcome = outcome(true)) {
  const calls: string[] = [];
  mockIPC((command) => {
    calls.push(command);
    switch (command) {
      case "plugin:dialog|open":
        return FILE;
      case "preview_workflow_on_file":
        return preview;
      case "run_workflow_on_file":
        return run;
      default:
        return undefined;
    }
  });
  return calls;
}

const plain = (text: string | null | undefined) => (text ?? "").replaceAll("\u00a0", " ");

describe("TrialPanel", () => {
  it("explains the simulation then runs for real", async () => {
    const calls = backend(outcome(false));
    const user = userEvent.setup();
    render(<TrialPanel draft={draft} workflowId="wf_01J8ZK3V9Q4X7M2N5P6R8T0W1Y" />);

    await user.click(screen.getByRole("button", { name: "Choisir un fichier…" }));

    expect(await screen.findByText("✓ Ce fichier est reconnu par le workflow.")).toBeTruthy();
    const conditions = screen.getAllByRole("listitem").map((item) => plain(item.textContent));
    expect(conditions[0]).toContain("le nom contient « facture »");
    expect(conditions[1]).toContain("date trouvée : 15 mars 2026");
    expect(conditions[2]).toBe("✓ remplie : l'extension est .pdf");
    expect(screen.getByText("En simulation (rien n'a été modifié)")).toBeTruthy();
    expect(screen.getByText(`→ ${moved}`)).toBeTruthy();
    expect(calls).not.toContain("run_workflow_on_file");

    await user.click(screen.getByRole("button", { name: "Exécuter pour de vrai" }));

    expect(await screen.findByText("Ce qui a été fait")).toBeTruthy();
    expect(plain(screen.getByText(/Terminé/).textContent)).toContain(moved);
    expect(screen.queryByRole("button", { name: "Exécuter pour de vrai" })).toBeNull();
  });

  it("explains a file that is not recognized and offers no real run", async () => {
    const notRecognized = outcome(false, {
      evaluation: {
        result: "no_match",
        conditions: [
          { path: "trigger.conditions[0]", result: "no_match", explanation: "text_search" },
          { path: "trigger.conditions[1]", result: "no_match", explanation: "no_date" },
          {
            path: "trigger.conditions[2]",
            result: "no_match",
            explanation: "extension",
            found: "docx",
          },
        ],
      },
    });
    backend(notRecognized);
    const user = userEvent.setup();
    render(<TrialPanel draft={draft} workflowId="wf_01J8ZK3V9Q4X7M2N5P6R8T0W1Y" />);

    await user.click(screen.getByRole("button", { name: "Choisir un fichier…" }));

    expect(await screen.findByText(/n'est pas reconnu/)).toBeTruthy();
    expect(screen.getByText(/aucune date trouvée/)).toBeTruthy();
    expect(screen.getByText(/extension\s:\s\.docx/)).toBeTruthy();
    expect(screen.getAllByText("non remplie :", { exact: false }).length).toBe(3);
    expect(screen.queryByRole("button", { name: "Exécuter pour de vrai" })).toBeNull();
  });

  it("shows a clear message for each failure", async () => {
    backend(
      outcome(false, {
        error: "document_encrypted",
        actions: [{ type: "convert", status: "failed", duration_ms: 12 }],
      }),
    );
    const user = userEvent.setup();
    render(
      <TrialPanel
        draft={{ ...draft, actions: [{ type: "convert", to: "png" }] }}
        workflowId="wf_01J8ZK3V9Q4X7M2N5P6R8T0W1Y"
      />,
    );

    await user.click(screen.getByRole("button", { name: "Choisir un fichier…" }));

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("protégé par un mot de passe");
    expect(screen.queryByRole("button", { name: "Exécuter pour de vrai" })).toBeNull();
  });

  it("only simulates an unsaved workflow", async () => {
    backend(outcome(false));
    const user = userEvent.setup();
    render(<TrialPanel draft={draft} />);

    await user.click(screen.getByRole("button", { name: "Choisir un fichier…" }));

    expect(await screen.findByText(/Enregistrez le workflow/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Exécuter pour de vrai" })).toBeNull();
  });

  it("reports a file that cannot be read", async () => {
    mockIPC((command) => {
      if (command === "plugin:dialog|open") {
        return FILE;
      }
      throw { code: "not_found", message_key: "errors.not_found" };
    });
    const user = userEvent.setup();
    render(<TrialPanel draft={draft} />);

    await user.click(screen.getByRole("button", { name: "Choisir un fichier…" }));

    expect((await screen.findByRole("alert")).textContent).toContain("introuvable");
  });
});
