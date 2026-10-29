// Étape « Récapitulatif » : le workflow en phrases (fonction partagée avec la
// liste), son nom proposé et modifiable, son activation.

import { useTranslation } from "react-i18next";
import { Switch } from "../../components/Switch";
import { TextField } from "../../components/TextField";
import { useWizardStore } from "../../state/wizard";
import { describeActions, describeTrigger } from "../summary";
import { issuesAt } from "./draft";
import { issueMessages, type WizardIssue } from "./WizardScreen";

export function SummaryStep({
  issues,
  proposedName,
}: {
  issues: readonly WizardIssue[];
  proposedName: string;
}) {
  const { t } = useTranslation();
  const session = useWizardStore((state) => state.session);
  const update = useWizardStore((state) => state.update);
  const editName = useWizardStore((state) => state.editName);
  if (!session) {
    return null;
  }
  const { draft } = session;
  const nameError = issueMessages(t, issuesAt(issues, "name")).join(" ") || undefined;

  return (
    <div className="wizard__content">
      <section className="wizard__summary" aria-label={t("workflows.wizard.summaryLabel")}>
        <p>
          <span className="workflow-row__label">{t("workflows.wizard.when")}</span>{" "}
          {describeTrigger(t, draft.trigger)}
        </p>
        <p>
          <span className="workflow-row__label">{t("workflows.wizard.then")}</span>{" "}
          {describeActions(t, draft.actions)}
        </p>
      </section>
      <TextField
        label={t("workflows.wizard.nameLabel")}
        hint={t("workflows.wizard.nameHint")}
        value={proposedName}
        error={nameError}
        maxLength={100}
        onChange={(event) => editName(event.target.value)}
      />
      <div className="choice__option">
        <Switch
          checked={draft.enabled}
          label={t("workflows.wizard.enableNow")}
          onCheckedChange={(enabled) => update((current) => ({ ...current, enabled }))}
        />{" "}
        <span aria-hidden="true">{t("workflows.wizard.enableNow")}</span>
      </div>
      <details className="wizard__advanced">
        <summary>{t("workflows.wizard.advanced")}</summary>
        <label className="choice__option">
          <input
            type="checkbox"
            checked={draft.continue_on_match}
            onChange={(event) =>
              update((current) => ({ ...current, continue_on_match: event.target.checked }))
            }
          />{" "}
          {t("workflows.wizard.continueOnMatch")}
        </label>
      </details>
    </div>
  );
}
