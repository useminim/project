// Étape « Que faire ? » : liste ordonnée d'actions. Renommer avec un modèle
// et son aperçu en direct, déplacer ou copier vers un dossier choisi dans la
// boîte de dialogue du système, convertir.

import { useEffect, useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { pickFolder, previewName } from "../../api";
import type { Action } from "../../bindings/Action";
import type { ConvertFormat } from "../../bindings/ConvertFormat";
import type { NamePreview } from "../../bindings/NamePreview";
import { Button } from "../../components/Button";
import { TextField } from "../../components/TextField";
import { useWizardStore } from "../../state/wizard";
import { defaultAction, issuesAt, type SimpleAction, simpleActions } from "./draft";
import { FieldErrors } from "./FieldErrors";
import { issueMessages, type WizardIssue } from "./WizardScreen";

const variables = ["original_name", "extension", "date_in_name", "file_date", "today"] as const;

/** Variables de date : insérées avec le format le plus courant en France. */
const dateVariables: ReadonlySet<string> = new Set(["date_in_name", "file_date", "today"]);

export function ActionsStep({ issues }: { issues: readonly WizardIssue[] }) {
  const { t } = useTranslation();
  const actions = useWizardStore((state) => state.session?.draft.actions);
  const update = useWizardStore((state) => state.update);
  if (!actions) {
    return null;
  }

  const setActions = (change: (actions: Action[]) => Action[]) =>
    update((draft) => ({ ...draft, actions: change(draft.actions) }));
  const move = (from: number, to: number) =>
    setActions((list) => {
      const next = [...list];
      const [item] = next.splice(from, 1);
      if (item) {
        next.splice(to, 0, item);
      }
      return next;
    });

  return (
    <div className="wizard__content">
      <FieldErrors issues={issues.filter((issue) => issue.path === "actions")} path="actions" />
      {actions.length === 0 ? (
        <p className="section__help">{t("workflows.wizard.noActions")}</p>
      ) : (
        <ol className="wizard__items">
          {actions.map((action, index) => (
            <ActionEditor
              // biome-ignore lint/suspicious/noArrayIndexKey: les actions n'ont pas d'identifiant ; l'éditeur suit leur position.
              key={`${index}-${action.type}`}
              number={index + 1}
              action={action}
              issues={issuesAt(issues, `actions[${index}]`)}
              onChange={(next) =>
                setActions((list) => list.map((item, i) => (i === index ? next : item)))
              }
              onRemove={() => setActions((list) => list.filter((_, i) => i !== index))}
              onMoveUp={index > 0 ? () => move(index, index - 1) : undefined}
              onMoveDown={index < actions.length - 1 ? () => move(index, index + 1) : undefined}
            />
          ))}
        </ol>
      )}
      <fieldset className="wizard__add">
        <legend className="field__label">{t("workflows.wizard.addAction")}</legend>
        <div className="wizard__add-buttons">
          {simpleActions.map((type) => (
            <Button key={type} onClick={() => setActions((list) => [...list, defaultAction(type)])}>
              + {t(`workflows.wizard.actionTypes.${type}`)}
            </Button>
          ))}
        </div>
      </fieldset>
    </div>
  );
}

interface ActionEditorProps {
  number: number;
  action: Action;
  issues: readonly WizardIssue[];
  onChange: (action: Action) => void;
  onRemove: () => void;
  onMoveUp?: () => void;
  onMoveDown?: () => void;
}

function ActionEditor({
  number,
  action,
  issues,
  onChange,
  onRemove,
  onMoveUp,
  onMoveDown,
}: ActionEditorProps) {
  const { t } = useTranslation();
  const type = action.type as SimpleAction;
  const title = t("workflows.wizard.actionNumber", {
    number,
    type: (simpleActions as readonly string[]).includes(type)
      ? t(`workflows.wizard.actionTypes.${type}`)
      : action.type,
  });
  const error = issueMessages(t, issues).join(" ") || undefined;

  return (
    <li className="wizard__item">
      <div className="wizard__item-header">
        <h2 className="wizard__item-title">{title}</h2>
        <div className="wizard__item-controls">
          <Button
            className="button--icon"
            aria-label={`${t("workflows.wizard.moveUp")} : ${title}`}
            disabled={!onMoveUp}
            onClick={onMoveUp}
          >
            <span aria-hidden="true">↑</span>
          </Button>
          <Button
            className="button--icon"
            aria-label={`${t("workflows.wizard.moveDown")} : ${title}`}
            disabled={!onMoveDown}
            onClick={onMoveDown}
          >
            <span aria-hidden="true">↓</span>
          </Button>
          <Button onClick={onRemove} aria-label={`${t("workflows.wizard.remove")} : ${title}`}>
            {t("workflows.wizard.remove")}
          </Button>
        </div>
      </div>
      {action.type === "rename" && (
        <TemplateEditor
          template={action.template}
          error={error}
          onChange={(template) => onChange({ ...action, template })}
        />
      )}
      {(action.type === "move" || action.type === "copy") && (
        <>
          <FolderPicker
            destination={action.destination}
            error={error}
            onChange={(destination) => onChange({ ...action, destination })}
          />
          {action.type === "move" && (
            <label className="choice__option">
              <input
                type="checkbox"
                checked={action.create_folders}
                onChange={(event) => onChange({ ...action, create_folders: event.target.checked })}
              />{" "}
              {t("workflows.wizard.createFolders")}
            </label>
          )}
        </>
      )}
      {action.type === "convert" && (
        <ConvertEditor action={action} error={error} onChange={onChange} />
      )}
    </li>
  );
}

function TemplateEditor({
  template,
  error,
  onChange,
}: {
  template: string;
  error?: string;
  onChange: (template: string) => void;
}) {
  const { t } = useTranslation();
  const input = useRef<HTMLInputElement>(null);
  const [sample, setSample] = useState("Facture 2026-03-15.pdf");
  const [preview, setPreview] = useState<NamePreview | null>(null);

  // Aperçu en direct, calculé par le moteur (même rendu que le renommage).
  useEffect(() => {
    let active = true;
    const timer = setTimeout(() => {
      previewName(template, sample)
        .then((result) => {
          if (active) {
            setPreview(result);
          }
        })
        .catch(() => {
          if (active) {
            setPreview(null);
          }
        });
    }, 200);
    return () => {
      active = false;
      clearTimeout(timer);
    };
  }, [template, sample]);

  const insert = (variable: string) => {
    const field = input.current;
    const text = dateVariables.has(variable) ? `{${variable}:YYYY-MM-DD}` : `{${variable}}`;
    const start = field?.selectionStart ?? template.length;
    const end = field?.selectionEnd ?? template.length;
    onChange(template.slice(0, start) + text + template.slice(end));
    requestAnimationFrame(() => {
      field?.focus();
      field?.setSelectionRange(start + text.length, start + text.length);
    });
  };

  return (
    <div className="template-editor">
      <TextField
        ref={input}
        label={t("workflows.wizard.templateLabel")}
        hint={t("workflows.wizard.templateHint")}
        value={template}
        error={error}
        spellCheck={false}
        onChange={(event) => onChange(event.target.value)}
      />
      <div className="template-editor__variables">
        {variables.map((variable) => (
          <Button
            key={variable}
            className="chip"
            aria-label={t("workflows.wizard.insertVariable", {
              variable: t(`workflows.wizard.variables.${variable}`),
            })}
            onClick={() => insert(variable)}
          >
            + {t(`workflows.wizard.variables.${variable}`)}
          </Button>
        ))}
      </div>
      <details className="wizard__advanced">
        <summary>{t("workflows.wizard.sampleLabel")}</summary>
        <TextField
          label={t("workflows.wizard.sampleLabel")}
          value={sample}
          onChange={(event) => setSample(event.target.value)}
        />
      </details>
      <p className="template-editor__preview" aria-live="polite">
        <span className="workflow-row__label">{t("workflows.wizard.previewLabel")}</span>{" "}
        {preview &&
          (preview.kind === "name"
            ? t("workflows.wizard.preview.name", { sample, name: preview.name })
            : t(
                `workflows.wizard.preview.${preview.kind}`,
                "variable" in preview ? { variable: preview.variable } : {},
              ))}
      </p>
    </div>
  );
}

function FolderPicker({
  destination,
  error,
  onChange,
}: {
  destination: string;
  error?: string;
  onChange: (destination: string) => void;
}) {
  const { t } = useTranslation();
  const id = useId();

  return (
    <div className="field">
      <span className="field__label" id={`${id}-label`}>
        {t("workflows.wizard.folderLabel")}
      </span>
      <div className="folder-picker">
        <output
          className="folder-picker__path"
          aria-labelledby={`${id}-label`}
          title={destination || undefined}
        >
          {destination || t("workflows.wizard.noFolder")}
        </output>
        <Button
          onClick={() => {
            pickFolder(t("workflows.wizard.folderDialogTitle"), destination || undefined)
              .then((chosen) => {
                if (chosen) {
                  onChange(chosen);
                }
              })
              .catch(() => {});
          }}
        >
          {destination ? t("workflows.wizard.changeFolder") : t("workflows.wizard.chooseFolder")}
        </Button>
      </div>
      {error && <p className="field__error">{error}</p>}
    </div>
  );
}

function ConvertEditor({
  action,
  error,
  onChange,
}: {
  action: Extract<Action, { type: "convert" }>;
  error?: string;
  onChange: (action: Action) => void;
}) {
  const { t } = useTranslation();
  const id = useId();
  const formats: ConvertFormat[] = ["pdf", "png", "jpeg"];
  const number = (value: string) => (value === "" ? undefined : Number(value));

  return (
    <>
      <div className="field">
        <label className="field__label" htmlFor={id}>
          {t("workflows.wizard.convertLabel")}
        </label>
        <select
          id={id}
          className="field__input"
          value={action.to}
          onChange={(event) =>
            onChange({ type: "convert", to: event.target.value as ConvertFormat })
          }
        >
          {formats.map((format) => (
            <option key={format} value={format}>
              {t(`workflows.wizard.convertFormats.${format}`)}
            </option>
          ))}
        </select>
      </div>
      {action.to !== "pdf" && (
        <details className="wizard__advanced" open={Boolean(error)}>
          <summary>{t("workflows.wizard.advanced")}</summary>
          <TextField
            type="number"
            label={t("workflows.wizard.dpiLabel")}
            placeholder="150"
            value={action.dpi ?? ""}
            onChange={(event) => onChange({ ...action, dpi: number(event.target.value) })}
          />
          {action.to === "jpeg" && (
            <TextField
              type="number"
              label={t("workflows.wizard.qualityLabel")}
              placeholder="85"
              value={action.quality ?? ""}
              onChange={(event) => onChange({ ...action, quality: number(event.target.value) })}
            />
          )}
        </details>
      )}
      {error && <p className="field__error">{error}</p>}
    </>
  );
}
