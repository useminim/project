// Explications d'un essai : condition désignée par un chemin d'évaluation, ce
// qui a été trouvé, message d'un code d'échec.

import type { TFunction } from "i18next";
import type { Condition } from "../../bindings/Condition";
import type { ConditionOutcome } from "../../bindings/ConditionOutcome";
import type { Trigger } from "../../bindings/Trigger";

/** Condition située à `path` (`trigger.conditions[1].conditions[0]`). */
export function conditionAt(trigger: Trigger, path: string): Condition | undefined {
  let conditions: Condition[] = trigger.conditions;
  let found: Condition | undefined;
  for (const match of path.matchAll(/conditions\[(\d+)\]/g)) {
    found = conditions[Number(match[1])];
    if (!found) {
      return undefined;
    }
    conditions = found.type === "group" ? found.conditions : [];
  }
  return found;
}

/** Profondeur d'imbrication d'une condition (0 au premier niveau). */
export function depthOf(path: string): number {
  return Math.max(0, [...path.matchAll(/conditions\[/g)].length - 1);
}

/** Ce qui a été observé pour une condition, ou `null` si le résultat suffit. */
export function describeObservation(
  t: TFunction,
  outcome: ConditionOutcome,
  language: string,
): string | null {
  switch (outcome.explanation) {
    case "date_found":
      return t("workflows.trial.observed.dateFound", {
        date: new Intl.DateTimeFormat(language, { dateStyle: "long", timeZone: "UTC" }).format(
          new Date(`${outcome.date}T00:00:00Z`),
        ),
      });
    case "no_date":
      return t("workflows.trial.observed.noDate");
    case "invalid_date_format":
      return t("workflows.trial.observed.invalidDateFormat");
    case "extension":
      // Utile seulement quand l'extension ne convient pas.
      return outcome.result === "match"
        ? null
        : t("workflows.trial.observed.extension", { extension: outcome.found });
    case "no_extension":
      return t("workflows.trial.observed.noExtension");
    case "not_available":
      return t("workflows.trial.observed.notAvailable");
    case "text_search":
    case "group":
      return null;
  }
}

/** Codes d'échec d'une exécution (`FailureCode` côté Rust). */
export const failureCodes = [
  "source_missing",
  "file_changed",
  "destination_missing",
  "missing_variable",
  "invalid_template",
  "empty_file_name",
  "too_many_collisions",
  "action_not_available",
  "copy_mismatch",
  "io",
  "unsupported_conversion",
  "document_encrypted",
  "document_unreadable",
  "document_too_large",
  "timeout",
  "converter_unavailable",
] as const;

/** Message d'un code d'échec ; un code inconnu donne un message générique. */
export function failureMessage(t: TFunction, code: string): string {
  return (failureCodes as readonly string[]).includes(code)
    ? t(`workflows.trial.failure.${code as (typeof failureCodes)[number]}`)
    : t("workflows.trial.failure.unknown");
}
