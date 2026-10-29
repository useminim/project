// Étape « Quels documents ? » : conditions sur le fichier, formulées en
// phrases, et choix « toutes » ou « au moins une ».

import { useId, useState } from "react";
import { useTranslation } from "react-i18next";
import type { Condition } from "../../bindings/Condition";
import type { MatchMode } from "../../bindings/MatchMode";
import { Button } from "../../components/Button";
import { TextField } from "../../components/TextField";
import { useWizardStore } from "../../state/wizard";
import {
  defaultCondition,
  type ExternalCondition,
  externalConditions,
  type FileType,
  fileTypeOf,
  fileTypes,
  issuesAt,
} from "./draft";
import { FieldErrors } from "./FieldErrors";
import { issueMessages, type WizardIssue } from "./WizardScreen";

export function DocumentsStep({ issues }: { issues: readonly WizardIssue[] }) {
  const { t } = useTranslation();
  const trigger = useWizardStore((state) => state.session?.draft.trigger);
  const update = useWizardStore((state) => state.update);
  if (!trigger) {
    return null;
  }

  const setConditions = (change: (conditions: Condition[]) => Condition[]) =>
    update((draft) => ({
      ...draft,
      trigger: { ...draft.trigger, conditions: change(draft.trigger.conditions) },
    }));
  const replace = (index: number, condition: Condition) =>
    setConditions((conditions) => conditions.map((item, i) => (i === index ? condition : item)));

  return (
    <div className="wizard__content">
      {trigger.conditions.length > 1 && (
        <fieldset className="choice">
          <legend className="field__label">{t("workflows.wizard.match.legend")}</legend>
          {(["all", "any"] as MatchMode[]).map((mode) => (
            <label key={mode} className="choice__option">
              <input
                type="radio"
                name="match"
                checked={trigger.match === mode}
                onChange={() =>
                  update((draft) => ({ ...draft, trigger: { ...draft.trigger, match: mode } }))
                }
              />{" "}
              {t(`workflows.wizard.match.${mode}`)}
            </label>
          ))}
        </fieldset>
      )}
      <FieldErrors issues={issues.filter((issue) => issue.path === "trigger")} path="trigger" />
      <ol className="wizard__items">
        {trigger.conditions.map((condition, index) => (
          <ConditionEditor
            // L'ordre des conditions ne change pas : l'indice suffit.
            // biome-ignore lint/suspicious/noArrayIndexKey: liste sans réordonnancement.
            key={index}
            number={index + 1}
            condition={condition}
            issues={issuesAt(issues, `trigger.conditions[${index}]`)}
            onChange={(next) => replace(index, next)}
            onRemove={
              trigger.conditions.length > 1
                ? () => setConditions((conditions) => conditions.filter((_, i) => i !== index))
                : undefined
            }
          />
        ))}
      </ol>
      <div>
        <Button
          onClick={() =>
            setConditions((conditions) => [...conditions, defaultCondition("file_name_contains")])
          }
        >
          {t("workflows.wizard.addCondition")}
        </Button>
      </div>
    </div>
  );
}

interface ConditionEditorProps {
  number: number;
  condition: Condition;
  issues: readonly WizardIssue[];
  onChange: (condition: Condition) => void;
  onRemove?: () => void;
}

function ConditionEditor({ number, condition, issues, onChange, onRemove }: ConditionEditorProps) {
  const { t } = useTranslation();
  const typeId = useId();
  const error = issueMessages(t, issues).join(" ") || undefined;
  const external = (externalConditions as readonly string[]).includes(condition.type);

  return (
    <li className="wizard__item">
      <div className="wizard__item-header">
        <h2 className="wizard__item-title">{t("workflows.wizard.conditionNumber", { number })}</h2>
        {onRemove && (
          <Button
            onClick={onRemove}
            aria-label={`${t("workflows.wizard.remove")} : ${t("workflows.wizard.conditionNumber", { number })}`}
          >
            {t("workflows.wizard.remove")}
          </Button>
        )}
      </div>
      {external ? (
        <div className="field">
          <label className="field__label" htmlFor={typeId}>
            {t("workflows.wizard.conditionType")}
          </label>
          <select
            id={typeId}
            className="field__input"
            value={condition.type}
            onChange={(event) =>
              onChange(defaultCondition(event.target.value as ExternalCondition))
            }
          >
            {externalConditions.map((type) => (
              <option key={type} value={type}>
                {t(`workflows.wizard.conditionTypes.${type}`)}
              </option>
            ))}
          </select>
        </div>
      ) : null}
      {condition.type === "file_name_contains" && (
        <>
          <TextField
            label={t("workflows.wizard.nameContainsLabel")}
            hint={t("workflows.wizard.nameContainsHint")}
            value={condition.value}
            error={error}
            onChange={(event) => onChange({ ...condition, value: event.target.value })}
          />
          <details className="wizard__advanced">
            <summary>{t("workflows.wizard.advanced")}</summary>
            <label className="choice__option">
              <input
                type="checkbox"
                checked={condition.case_sensitive}
                onChange={(event) =>
                  onChange({ ...condition, case_sensitive: event.target.checked })
                }
              />{" "}
              {t("workflows.wizard.caseSensitive")}
            </label>
          </details>
        </>
      )}
      {condition.type === "file_name_matches_date" && (
        <details className="wizard__advanced" open={Boolean(condition.format) || Boolean(error)}>
          <summary>{t("workflows.wizard.advanced")}</summary>
          <TextField
            label={t("workflows.wizard.dateFormatLabel")}
            hint={t("workflows.wizard.dateFormatHint")}
            value={condition.format ?? ""}
            error={error}
            onChange={(event) => {
              const format = event.target.value;
              onChange(format ? { ...condition, format } : { type: condition.type });
            }}
          />
        </details>
      )}
      {condition.type === "file_extension_is" && (
        <FileTypeEditor
          values={condition.values}
          error={error}
          onChange={(values) => onChange({ ...condition, values })}
        />
      )}
      {!external && <p className="section__help">{issueMessages(t, issues).join(" ")}</p>}
    </li>
  );
}

function FileTypeEditor({
  values,
  error,
  onChange,
}: {
  values: string[];
  error?: string;
  onChange: (values: string[]) => void;
}) {
  const { t } = useTranslation();
  const id = useId();
  // « Autres extensions » reste choisi même si la saisie retombe sur un type connu.
  const [custom, setCustom] = useState(fileTypeOf(values) === undefined);
  const selected: FileType | "custom" = custom ? "custom" : (fileTypeOf(values) ?? "custom");

  return (
    <>
      <div className="field">
        <label className="field__label" htmlFor={id}>
          {t("workflows.wizard.fileTypeLabel")}
        </label>
        <select
          id={id}
          className="field__input"
          value={selected}
          onChange={(event) => {
            const type = event.target.value as FileType | "custom";
            setCustom(type === "custom");
            onChange(type === "custom" ? [] : [...fileTypes[type]]);
          }}
        >
          {(Object.keys(fileTypes) as FileType[]).map((type) => (
            <option key={type} value={type}>
              {t(`workflows.wizard.fileTypes.${type}`)}
            </option>
          ))}
          <option value="custom">{t("workflows.wizard.fileTypes.custom")}</option>
        </select>
      </div>
      {selected === "custom" && (
        <TextField
          label={t("workflows.wizard.extensionsLabel")}
          hint={t("workflows.wizard.extensionsHint")}
          defaultValue={values.join(", ")}
          error={error}
          onChange={(event) =>
            onChange(
              event.target.value
                .split(",")
                .map((value) => value.trim().replace(/^\./, ""))
                .filter(Boolean),
            )
          }
        />
      )}
      {selected !== "custom" && error && <p className="field__error">{error}</p>}
    </>
  );
}
