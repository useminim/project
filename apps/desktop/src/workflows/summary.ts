// Résumé en langage courant d'un workflow : « Le nom contient « facture » »,
// « Renommer, puis déplacer vers Comptabilité ». Fonctions pures, partagées par
// la liste des workflows et le récapitulatif du wizard.

import type { TFunction } from "i18next";
import type { Action } from "../bindings/Action";
import type { Condition } from "../bindings/Condition";
import type { MatchMode } from "../bindings/MatchMode";
import type { Trigger } from "../bindings/Trigger";

/** Dernier dossier d'un chemin Windows, macOS ou Linux (« Comptabilité »). */
export function folderName(path: string): string {
  const parts = path.split(/[\\/]+/).filter((part) => part.length > 0);
  return parts.at(-1) ?? path;
}

function joinConditions(t: TFunction, match: MatchMode, conditions: Condition[]): string {
  return conditions
    .map((condition) =>
      condition.type === "group"
        ? t("workflows.summary.group", { conditions: describeConditions(t, condition) })
        : describeCondition(t, condition),
    )
    .join(t(`workflows.summary.match.${match}`));
}

function describeConditions(t: TFunction, group: Extract<Condition, { type: "group" }>): string {
  return joinConditions(t, group.match, group.conditions);
}

/** Une condition, en une phrase sans majuscule initiale (« le nom contient « facture » »). */
export function describeCondition(t: TFunction, condition: Condition): string {
  const exactCase = (sensitive: boolean) =>
    sensitive ? t("workflows.summary.condition.exactCase") : "";
  switch (condition.type) {
    case "file_name_contains":
      return (
        t("workflows.summary.condition.fileNameContains", { value: condition.value }) +
        exactCase(condition.case_sensitive)
      );
    case "file_name_matches_date":
      return condition.format
        ? t("workflows.summary.condition.fileNameDateFormat", { format: condition.format })
        : t("workflows.summary.condition.fileNameDate");
    case "file_extension_is":
      return t("workflows.summary.condition.extension", {
        extensions: condition.values
          .map((value) => `.${value.toLowerCase()}`)
          .join(t("workflows.summary.match.any")),
      });
    case "content_contains_text":
      return (
        t("workflows.summary.condition.contentContains", { value: condition.value }) +
        exactCase(condition.case_sensitive)
      );
    case "content_has_field":
      return t("workflows.summary.condition.hasField", { field: condition.field });
    case "content_contains_table":
      return condition.min_rows
        ? t("workflows.summary.condition.tableMinRows", { count: condition.min_rows })
        : t("workflows.summary.condition.table");
    case "similar_to_examples":
      return t("workflows.summary.condition.similar");
    case "group":
      return t("workflows.summary.group", { conditions: describeConditions(t, condition) });
  }
}

/** Le déclencheur, en une phrase (« Le nom contient « facture » et l'extension est .pdf »). */
export function describeTrigger(t: TFunction, trigger: Trigger): string {
  if (trigger.conditions.length === 0) {
    return t("workflows.summary.noCondition");
  }
  return capitalize(joinConditions(t, trigger.match, trigger.conditions));
}

const templateVariables = [
  "original_name",
  "extension",
  "date_in_name",
  "file_date",
  "today",
] as const;

/**
 * Modèle de nom lisible : chaque variable connue devient son nom en clair
 * (« [Date dans le nom]_[Nom d'origine] »), les autres restent telles quelles.
 */
export function readableTemplate(t: TFunction, template: string): string {
  return template.replace(/\{([a-z_]+)(?::[^}]*)?\}/g, (variable, name: string) =>
    (templateVariables as readonly string[]).includes(name)
      ? `[${t(`workflows.wizard.variables.${name as (typeof templateVariables)[number]}`)}]`
      : variable,
  );
}

/** Une action, sans majuscule initiale (« déplacer vers Comptabilité »). */
export function describeAction(t: TFunction, action: Action): string {
  switch (action.type) {
    case "rename":
      return t("workflows.summary.action.rename", {
        template: readableTemplate(t, action.template),
      });
    case "move":
      return t("workflows.summary.action.move", { folder: folderName(action.destination) });
    case "copy":
      return t("workflows.summary.action.copy", { folder: folderName(action.destination) });
    case "convert":
      return t(`workflows.summary.action.convert.${action.to}`);
    case "extract_fields":
      return t(`workflows.summary.action.extract.${action.output}`, {
        count: action.fields.length,
      });
  }
}

/** Les actions dans l'ordre (« Renommer en « … », puis déplacer vers Comptabilité »). */
export function describeActions(t: TFunction, actions: Action[]): string {
  if (actions.length === 0) {
    return t("workflows.summary.noAction");
  }
  return capitalize(
    actions.map((action) => describeAction(t, action)).join(t("workflows.summary.then")),
  );
}

function capitalize(text: string): string {
  return text.charAt(0).toLocaleUpperCase() + text.slice(1);
}
