// Brouillons du wizard : valeurs par défaut, nom proposé et répartition des
// problèmes de validation entre les étapes.

import type { TFunction } from "i18next";
import type { Action } from "../../bindings/Action";
import type { Condition } from "../../bindings/Condition";
import type { Workflow } from "../../bindings/Workflow";
import type { WorkflowDraft } from "../../bindings/WorkflowDraft";
import type { WizardStep } from "../../state/wizard";

/** Conditions proposées par le wizard v1 : celles qui portent sur le fichier. */
export const externalConditions = [
  "file_name_contains",
  "file_name_matches_date",
  "file_extension_is",
] as const;

export type ExternalCondition = (typeof externalConditions)[number];

/** Actions proposées par le wizard v1. */
export const simpleActions = ["rename", "move", "copy", "convert"] as const;

export type SimpleAction = (typeof simpleActions)[number];

/** Types de fichiers proposés, et leurs extensions. */
export const fileTypes = {
  pdf: ["pdf"],
  image: ["png", "jpg", "jpeg"],
  word: ["docx"],
  excel: ["xlsx"],
} as const;

export type FileType = keyof typeof fileTypes;

/** Type de fichier correspondant exactement à des extensions, s'il y en a un. */
export function fileTypeOf(values: readonly string[]): FileType | undefined {
  const sorted = [...values]
    .map((value) => value.toLowerCase())
    .sort()
    .join(",");
  return (Object.keys(fileTypes) as FileType[]).find(
    (type) => [...fileTypes[type]].sort().join(",") === sorted,
  );
}

export function defaultCondition(type: ExternalCondition): Condition {
  switch (type) {
    case "file_name_contains":
      return { type, value: "", case_sensitive: false };
    case "file_name_matches_date":
      return { type };
    case "file_extension_is":
      return { type, values: [...fileTypes.pdf] };
  }
}

export function defaultAction(type: SimpleAction): Action {
  switch (type) {
    case "rename":
      return { type, template: "{date_in_name:YYYY-MM-DD}_{original_name}.{extension}" };
    case "move":
      return { type, destination: "", create_folders: false };
    case "copy":
      return { type, destination: "" };
    case "convert":
      return { type, to: "pdf" };
  }
}

export function newDraft(): WorkflowDraft {
  return {
    name: "",
    enabled: true,
    continue_on_match: false,
    trigger: {
      match: "all",
      conditions: [defaultCondition("file_name_contains")],
      source: "manual",
    },
    actions: [],
  };
}

export function draftOf(workflow: Workflow): WorkflowDraft {
  return {
    name: workflow.name,
    enabled: workflow.enabled,
    continue_on_match: workflow.continue_on_match,
    trigger: workflow.trigger,
    actions: workflow.actions,
  };
}

/** Nom proposé d'après la première condition (« Facture », « Fichiers PDF »). */
export function proposeName(t: TFunction, draft: WorkflowDraft): string {
  for (const condition of draft.trigger.conditions) {
    if (condition.type === "file_name_contains" && condition.value.trim()) {
      const value = condition.value.trim();
      return value.charAt(0).toLocaleUpperCase() + value.slice(1);
    }
    if (condition.type === "file_extension_is") {
      const type = fileTypeOf(condition.values);
      if (type) {
        return t(`workflows.wizard.proposedName.${type}`);
      }
    }
  }
  return t("workflows.wizard.proposedName.default");
}

/** Étape où se corrige le problème situé à `path`. */
export function stepOfPath(path: string): WizardStep {
  if (path.startsWith("trigger")) {
    return "documents";
  }
  if (path.startsWith("actions")) {
    return "actions";
  }
  return "summary";
}

/** Problèmes situés exactement à `path` ou à l'intérieur. */
export function issuesAt<T extends { path: string }>(issues: readonly T[], path: string): T[] {
  return issues.filter(
    (issue) =>
      issue.path === path || issue.path.startsWith(`${path}.`) || issue.path.startsWith(`${path}[`),
  );
}
