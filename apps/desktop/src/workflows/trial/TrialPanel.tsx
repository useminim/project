// Essai d'un workflow sur un fichier choisi : simulation (rien n'est
// modifié), explication condition par condition, puis exécution réelle si le
// workflow est enregistré et reconnaît le fichier.

import type { ParseKeys } from "i18next";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { pickFile, previewWorkflowOnFile, runWorkflowOnFile, toCommandError } from "../../api";
import type { ActionRecord } from "../../bindings/ActionRecord";
import type { TrialOutcome } from "../../bindings/TrialOutcome";
import type { WorkflowDraft } from "../../bindings/WorkflowDraft";
import type { WorkflowId } from "../../bindings/WorkflowId";
import { Button } from "../../components/Button";
import { Spinner } from "../../components/Spinner";
import { describeAction, describeCondition } from "../summary";
import { conditionAt, depthOf, describeObservation, failureMessage } from "./explain";

interface TrialPanelProps {
  draft: WorkflowDraft;
  /** Workflow enregistré, à jour : l'exécution réelle est alors proposée. */
  workflowId?: WorkflowId;
}

function fileName(path: string): string {
  return path.split(/[\\/]/).at(-1) ?? path;
}

export function TrialPanel({ draft, workflowId }: TrialPanelProps) {
  const { t, i18n } = useTranslation();
  const [path, setPath] = useState<string | null>(null);
  const [preview, setPreview] = useState<TrialOutcome | null>(null);
  const [result, setResult] = useState<TrialOutcome | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const simulate = async (chosen: string) => {
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      setPreview(await previewWorkflowOnFile(draft, chosen));
    } catch (reason) {
      setPreview(null);
      setError(t(toCommandError(reason).message_key as ParseKeys));
    } finally {
      setBusy(false);
    }
  };

  const choose = async () => {
    const chosen = await pickFile(t("workflows.trial.pickTitle")).catch(() => null);
    if (chosen) {
      setPath(chosen);
      await simulate(chosen);
    }
  };

  const runForReal = async () => {
    if (!workflowId || !path) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      setResult(await runWorkflowOnFile(workflowId, path));
    } catch (reason) {
      setError(t(toCommandError(reason).message_key as ParseKeys));
    } finally {
      setBusy(false);
    }
  };

  const canRun =
    workflowId !== undefined &&
    preview?.evaluation.result === "match" &&
    preview.error === undefined &&
    result === null;

  return (
    <section className="trial" aria-labelledby="trial-title">
      <h2 id="trial-title" className="trial__title">
        {t("workflows.trial.title")}
      </h2>
      <p className="section__help">{t("workflows.trial.intro")}</p>
      <div className="folder-picker">
        <output className="folder-picker__path" title={path ?? undefined}>
          {path ? fileName(path) : t("workflows.trial.noFile")}
        </output>
        <Button onClick={() => void choose()} disabled={busy}>
          {path ? t("workflows.trial.changeFile") : t("workflows.trial.chooseFile")}
        </Button>
        {path && (
          <Button onClick={() => void simulate(path)} disabled={busy}>
            {t("workflows.trial.again")}
          </Button>
        )}
      </div>
      <div aria-live="polite">
        {busy && <Spinner />}
        {error && (
          <p className="field__error" role="alert">
            {error}
          </p>
        )}
        {preview && !busy && (
          <Outcome
            outcome={result ?? preview}
            draft={draft}
            language={i18n.language}
            heading={result ? t("workflows.trial.doneTitle") : t("workflows.trial.simulationTitle")}
          />
        )}
      </div>
      {preview && !busy && workflowId === undefined && (
        <p className="section__help">{t("workflows.trial.saveToRun")}</p>
      )}
      {canRun && (
        <div>
          <Button variant="primary" onClick={() => void runForReal()}>
            {t("workflows.trial.run")}
          </Button>
        </div>
      )}
    </section>
  );
}

function Outcome({
  outcome,
  draft,
  language,
  heading,
}: {
  outcome: TrialOutcome;
  draft: WorkflowDraft;
  language: string;
  heading: string;
}) {
  const { t } = useTranslation();
  const verdict = outcome.evaluation.result;

  return (
    <div className="trial__outcome">
      <p className={`trial__verdict trial__verdict--${verdict}`}>
        {t(`workflows.trial.verdict.${verdict}`)}
      </p>
      <ul className="trial__conditions">
        {outcome.evaluation.conditions.map((condition) => {
          const described = conditionAt(draft.trigger, condition.path);
          const observation = describeObservation(t, condition, language);
          return (
            <li
              key={condition.path}
              className="trial__condition"
              style={{ marginInlineStart: `${depthOf(condition.path) * 1.5}rem` }}
            >
              <span className={`trial__mark trial__mark--${condition.result}`} aria-hidden="true">
                {condition.result === "match" ? "✓" : condition.result === "no_match" ? "✗" : "?"}
              </span>{" "}
              <span className="visually-hidden">
                {t(`workflows.trial.result.${condition.result}`)} :{" "}
              </span>
              {described ? describeCondition(t, described) : condition.path}
              {observation && <span className="trial__observed"> — {observation}</span>}
            </li>
          );
        })}
      </ul>
      {outcome.actions.length > 0 && (
        <>
          <h3 className="trial__subtitle">{heading}</h3>
          <ol className="trial__actions">
            {outcome.actions.map((record, index) => {
              const action = draft.actions[index];
              return (
                <ActionResult
                  // biome-ignore lint/suspicious/noArrayIndexKey: une ligne par action, dans l'ordre.
                  key={index}
                  label={action ? describeAction(t, action) : record.type}
                  record={record}
                />
              );
            })}
          </ol>
        </>
      )}
      {outcome.error && (
        <p className="field__error" role="alert">
          {failureMessage(t, outcome.error)}
          {outcome.executed && ` ${t("workflows.trial.rolledBack")}`}
        </p>
      )}
      {outcome.executed && !outcome.error && (
        <p className="trial__done">{t("workflows.trial.done", { location: outcome.location })}</p>
      )}
    </div>
  );
}

function ActionResult({ label, record }: { label: string; record: ActionRecord }) {
  const { t } = useTranslation();
  const operations = record.operations ?? [];

  return (
    <li className="trial__action">
      <span className="trial__action-label">
        {label.charAt(0).toLocaleUpperCase() + label.slice(1)}
      </span>{" "}
      <span className={`trial__status trial__status--${record.status}`}>
        ({t(`workflows.trial.status.${record.status}`)})
      </span>
      {operations.length === 0 && record.status === "success" && (
        <span className="trial__observed"> — {t("workflows.trial.unchanged")}</span>
      )}
      {operations.length > 0 && (
        <ul className="trial__files">
          {operations.map((operation) => (
            <li key={operation.to} className="trial__file">
              → {operation.to}
            </li>
          ))}
        </ul>
      )}
    </li>
  );
}
