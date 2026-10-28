import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Workflow } from "../bindings/Workflow";
import type { WorkflowDraft } from "../bindings/WorkflowDraft";
import { ApiError } from "./invoke";
import {
  createWorkflow,
  deleteWorkflow,
  getWorkflow,
  listWorkflows,
  onWorkflowsChanged,
  reorderWorkflows,
  setWorkflowEnabled,
  updateWorkflow,
  validateWorkflow,
  WORKFLOWS_CHANGED,
} from "./workflows";

afterEach(() => {
  clearMocks();
});

const draft: WorkflowDraft = {
  name: "Factures",
  enabled: true,
  continue_on_match: false,
  trigger: {
    match: "all",
    conditions: [{ type: "file_name_contains", value: "facture", case_sensitive: false }],
    source: "manual",
  },
  actions: [{ type: "rename", template: "{date}.pdf" }],
};

const workflow: Workflow = {
  ...draft,
  schema_version: 1,
  id: "wf_01J8ZK3V9Q4X7M2N5P6R8T0W1Y",
  version: 1,
  priority: 10,
  owner: { kind: "user", id: "usr_00000000000000000000000000" },
  created_at: "2026-10-30T09:00:00Z",
  updated_at: "2026-10-30T09:00:00Z",
};

/** Enregistre les appels et répond avec `reply`. */
function recordCalls(reply: unknown) {
  const calls: Array<{ command: string; args: unknown }> = [];
  mockIPC((command, args) => {
    calls.push({ command, args });
    return reply;
  });
  return calls;
}

describe("workflow commands", () => {
  it("calls each command with its arguments", async () => {
    const calls = recordCalls(workflow);

    await listWorkflows();
    await getWorkflow(workflow.id);
    await validateWorkflow(draft);
    await createWorkflow(draft);
    await updateWorkflow(workflow.id, 1, draft);
    await deleteWorkflow(workflow.id);
    await setWorkflowEnabled(workflow.id, false);
    await reorderWorkflows([workflow.id]);

    expect(calls).toEqual([
      { command: "list_workflows", args: {} },
      { command: "get_workflow", args: { id: workflow.id } },
      { command: "validate_workflow", args: { draft } },
      { command: "create_workflow", args: { draft } },
      { command: "update_workflow", args: { id: workflow.id, expectedVersion: 1, draft } },
      { command: "delete_workflow", args: { id: workflow.id } },
      { command: "set_workflow_enabled", args: { id: workflow.id, enabled: false } },
      { command: "reorder_workflows", args: { ordered: [workflow.id] } },
    ]);
  });

  it("returns the typed result", async () => {
    recordCalls([workflow]);

    await expect(listWorkflows()).resolves.toEqual([workflow]);
  });

  it("reports a version conflict as a typed error", async () => {
    mockIPC(() => {
      throw { code: "conflict", message_key: "errors.conflict" };
    });

    const error = await updateWorkflow(workflow.id, 1, draft).catch((reason: unknown) => reason);

    expect(error).toBeInstanceOf(ApiError);
    expect((error as ApiError).error.code).toBe("conflict");
  });

  it("notifies changes until unsubscribed", async () => {
    mockIPC(() => undefined, { shouldMockEvents: true });
    const onChange = vi.fn();

    const unlisten = await onWorkflowsChanged(onChange);
    await emit(WORKFLOWS_CHANGED);
    unlisten();
    await emit(WORKFLOWS_CHANGED);

    expect(onChange).toHaveBeenCalledTimes(1);
  });
});
