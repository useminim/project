// Gestion des workflows : une fonction typée par commande Tauri
// (`src-tauri/src/commands/workflows.rs`), et l'abonnement à l'événement
// émis après chaque modification.

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { NamePreview } from "../bindings/NamePreview";
import type { TrialOutcome } from "../bindings/TrialOutcome";
import type { ValidationError } from "../bindings/ValidationError";
import type { Workflow } from "../bindings/Workflow";
import type { WorkflowDraft } from "../bindings/WorkflowDraft";
import type { WorkflowId } from "../bindings/WorkflowId";
import { invokeCommand } from "./invoke";

/** Nom de l'événement émis après chaque modification des workflows. */
export const WORKFLOWS_CHANGED = "workflows_changed";

/** Workflows dans l'ordre d'évaluation (priorité croissante). */
export function listWorkflows(): Promise<Workflow[]> {
  return invokeCommand<Workflow[]>("list_workflows");
}

/** Un workflow ; erreur `not_found` s'il n'existe pas. */
export function getWorkflow(id: WorkflowId): Promise<Workflow> {
  return invokeCommand<Workflow>("get_workflow", { id });
}

/** Problèmes de validation d'un brouillon, sans rien enregistrer (vide s'il est valide). */
export function validateWorkflow(draft: WorkflowDraft): Promise<ValidationError[]> {
  return invokeCommand<ValidationError[]>("validate_workflow", { draft });
}

/** Crée un workflow, placé après les autres. */
export function createWorkflow(draft: WorkflowDraft): Promise<Workflow> {
  return invokeCommand<Workflow>("create_workflow", { draft });
}

/**
 * Remplace le contenu d'un workflow. `expectedVersion` est la version lue :
 * si le workflow a changé depuis, l'erreur `conflict` demande de le relire.
 */
export function updateWorkflow(
  id: WorkflowId,
  expectedVersion: number,
  draft: WorkflowDraft,
): Promise<Workflow> {
  return invokeCommand<Workflow>("update_workflow", { id, expectedVersion, draft });
}

/** Supprime un workflow. */
export function deleteWorkflow(id: WorkflowId): Promise<void> {
  return invokeCommand<void>("delete_workflow", { id });
}

/** Active ou désactive un workflow. */
export function setWorkflowEnabled(id: WorkflowId, enabled: boolean): Promise<Workflow> {
  return invokeCommand<Workflow>("set_workflow_enabled", { id, enabled });
}

/** Réordonne les workflows (chacun une fois exactement) ; renvoie la liste dans le nouvel ordre. */
export function reorderWorkflows(ordered: WorkflowId[]): Promise<Workflow[]> {
  return invokeCommand<Workflow[]>("reorder_workflows", { ordered });
}

/** Appelle `onChange` après chaque modification des workflows ; renvoie la fonction de désabonnement. */
export function onWorkflowsChanged(onChange: () => void): Promise<UnlistenFn> {
  return listen(WORKFLOWS_CHANGED, () => onChange());
}

/** Nom que recevrait un fichier nommé `sample` avec le modèle `template` (aperçu du wizard). */
export function previewName(template: string, sample: string): Promise<NamePreview> {
  return invokeCommand<NamePreview>("preview_name", { template, sample });
}

/** Ce que ferait le brouillon sur le fichier `path`, sans rien modifier. */
export function previewWorkflowOnFile(draft: WorkflowDraft, path: string): Promise<TrialOutcome> {
  return invokeCommand<TrialOutcome>("preview_workflow_on_file", { draft, path });
}

/** Exécute pour de vrai le workflow enregistré `id` sur le fichier `path`. */
export function runWorkflowOnFile(id: WorkflowId, path: string): Promise<TrialOutcome> {
  return invokeCommand<TrialOutcome>("run_workflow_on_file", { id, path });
}
