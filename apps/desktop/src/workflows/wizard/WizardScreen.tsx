// Wizard de création et de modification d'un workflow, en trois étapes :
// « Quels documents ? », « Que faire ? », « Récapitulatif ». La validation est
// celle du moteur (`validate_workflow`) : chaque problème s'affiche à l'étape
// et au champ que désigne son chemin.

import type { ParseKeys } from "i18next";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  createWorkflow,
  previewName,
  toCommandError,
  updateWorkflow,
  validateWorkflow,
} from "../../api";
import type { ValidationError } from "../../bindings/ValidationError";
import type { WorkflowDraft } from "../../bindings/WorkflowDraft";
import { Button } from "../../components/Button";
import { ConfirmDialog } from "../../components/ConfirmDialog";
import { useUiStore } from "../../state/ui";
import { useWizardStore, type WizardStep, wizardSteps } from "../../state/wizard";
import { ActionsStep } from "./ActionsStep";
import { DocumentsStep } from "./DocumentsStep";
import { proposeName, stepOfPath } from "./draft";
import { SummaryStep } from "./SummaryStep";

/** Problème signalé par le wizard lui-même, avant la validation du moteur. */
export type WizardIssue = ValidationError | { path: string; code: "invalid_template" };

/** Modèles de nom illisibles : le moteur refuserait le brouillon entier. */
async function templateIssues(draft: WorkflowDraft): Promise<WizardIssue[]> {
  const issues: WizardIssue[] = [];
  for (const [index, action] of draft.actions.entries()) {
    if (action.type === "rename") {
      const preview = await previewName(action.template, "exemple.pdf");
      if (preview.kind === "invalid_template") {
        issues.push({ path: `actions[${index}]`, code: "invalid_template" });
      }
    }
  }
  return issues;
}

export function WizardScreen() {
  const { t } = useTranslation();
  const session = useWizardStore((state) => state.session);
  const goTo = useWizardStore((state) => state.goTo);
  const close = useWizardStore((state) => state.close);
  const showToast = useUiStore((state) => state.showToast);
  const [issues, setIssues] = useState<WizardIssue[]>([]);
  const [busy, setBusy] = useState(false);
  const [confirmAbandon, setConfirmAbandon] = useState(false);
  const headingRef = useRef<HTMLHeadingElement>(null);
  const step = session?.step;

  // À chaque étape, le focus va sur son titre (annoncé par les lecteurs d'écran).
  useEffect(() => {
    if (step) {
      headingRef.current?.focus();
    }
  }, [step]);

  if (!session) {
    return null;
  }
  const index = wizardSteps.indexOf(session.step);
  const isLast = index === wizardSteps.length - 1;

  /** Brouillon tel qu'il sera enregistré : nom proposé s'il n'a pas été saisi. */
  const finalDraft = (): WorkflowDraft =>
    session.nameEdited || session.mode === "edit"
      ? session.draft
      : { ...session.draft, name: proposeName(t, session.draft) };

  /** Problèmes du brouillon, ou `null` si la validation n'a pas pu se faire. */
  const check = async (draft: WorkflowDraft): Promise<WizardIssue[] | null> => {
    try {
      const local = await templateIssues(draft);
      return local.length > 0 ? local : await validateWorkflow(draft);
    } catch (reason) {
      showToast("error", toCommandError(reason).message_key as ParseKeys);
      return null;
    }
  };

  const next = async () => {
    setBusy(true);
    const found = await check(finalDraft());
    setBusy(false);
    if (found === null) {
      return;
    }
    // On ne bloque que sur les problèmes de l'étape en cours ou des précédentes.
    const blocking = found.filter((issue) => wizardSteps.indexOf(stepOfPath(issue.path)) <= index);
    setIssues(blocking);
    const first = blocking[0];
    if (first) {
      goTo(stepOfPath(first.path));
      showToast("error", "workflows.wizard.fixErrors");
      return;
    }
    goTo(wizardSteps[index + 1] as WizardStep);
  };

  const save = async () => {
    const draft = finalDraft();
    setBusy(true);
    const found = await check(draft);
    if (found === null || found.length > 0) {
      setBusy(false);
      if (found?.[0]) {
        setIssues(found);
        goTo(stepOfPath(found[0].path));
        showToast("error", "workflows.wizard.fixErrors");
      }
      return;
    }
    try {
      if (session.mode === "edit" && session.workflowId && session.version !== undefined) {
        await updateWorkflow(session.workflowId, session.version, draft);
        showToast("success", "workflows.wizard.saved");
      } else {
        await createWorkflow(draft);
        showToast("success", "workflows.wizard.created");
      }
      close();
    } catch (reason) {
      const error = toCommandError(reason);
      showToast(
        "error",
        error.code === "conflict" ? "workflows.wizard.conflict" : (error.message_key as ParseKeys),
      );
    } finally {
      setBusy(false);
    }
  };

  const title =
    session.mode === "edit"
      ? t("workflows.wizard.titleEdit", { name: session.draft.name })
      : t("workflows.wizard.titleCreate");
  const stepTitle = t(`workflows.wizard.steps.${session.step}`);

  return (
    <section className="section wizard" aria-labelledby="wizard-title">
      <header className="section__header">
        <p id="wizard-title" className="section__title">
          {title}
        </p>
      </header>
      <ol className="wizard__progress" aria-label={t("workflows.wizard.progress")}>
        {wizardSteps.map((item, position) => (
          <li
            key={item}
            className="wizard__progress-step"
            aria-current={item === session.step ? "step" : undefined}
          >
            <span className="wizard__progress-number" aria-hidden="true">
              {position + 1}
            </span>{" "}
            {t(`workflows.wizard.steps.${item}`)}
          </li>
        ))}
      </ol>
      <form
        className="wizard__step"
        onSubmit={(event) => {
          event.preventDefault();
          void (isLast ? save() : next());
        }}
        noValidate
      >
        <h1 ref={headingRef} tabIndex={-1} className="wizard__step-title">
          <span className="visually-hidden">
            {t("workflows.wizard.stepAnnounce", {
              number: index + 1,
              total: wizardSteps.length,
              title: stepTitle,
            })}
          </span>
          <span aria-hidden="true">{stepTitle}</span>
        </h1>
        <p className="section__help">{t(`workflows.wizard.intro.${session.step}`)}</p>
        {session.step === "documents" && <DocumentsStep issues={issues} />}
        {session.step === "actions" && <ActionsStep issues={issues} />}
        {session.step === "summary" && (
          <SummaryStep issues={issues} proposedName={finalDraft().name} />
        )}
        <div className="wizard__footer">
          <Button
            onClick={() => (session.dirty ? setConfirmAbandon(true) : close())}
            disabled={busy}
          >
            {t("workflows.wizard.cancel")}
          </Button>
          <div className="wizard__footer-main">
            {index > 0 && (
              <Button onClick={() => goTo(wizardSteps[index - 1] as WizardStep)} disabled={busy}>
                {t("workflows.wizard.back")}
              </Button>
            )}
            <Button type="submit" variant="primary" disabled={busy}>
              {isLast
                ? t(session.mode === "edit" ? "workflows.wizard.save" : "workflows.wizard.create")
                : t("workflows.wizard.next")}
            </Button>
          </div>
        </div>
      </form>
      <ConfirmDialog
        open={confirmAbandon}
        onOpenChange={setConfirmAbandon}
        title={t("workflows.wizard.abandonTitle")}
        description={t("workflows.wizard.abandonDescription")}
        confirmLabel={t("workflows.wizard.abandonConfirm")}
        destructive
        onConfirm={close}
      />
    </section>
  );
}

/** Messages des problèmes, traduits. */
export function issueMessages(
  t: ReturnType<typeof useTranslation>["t"],
  issues: readonly WizardIssue[],
): string[] {
  return issues.map((issue) =>
    issue.code === "invalid_template"
      ? t("workflows.wizard.invalidTemplate")
      : t(`workflows.validation.${issue.code}`, issue as Record<string, unknown>),
  );
}
