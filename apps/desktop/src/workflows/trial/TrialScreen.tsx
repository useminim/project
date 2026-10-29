// Essai d'un workflow enregistré, ouvert depuis la liste.

import { useTranslation } from "react-i18next";
import { Button } from "../../components/Button";
import { useTrialStore } from "../../state/trial";
import { draftOf } from "../wizard/draft";
import { TrialPanel } from "./TrialPanel";

export function TrialScreen() {
  const { t } = useTranslation();
  const workflow = useTrialStore((state) => state.workflow);
  const close = useTrialStore((state) => state.close);
  if (!workflow) {
    return null;
  }

  return (
    <section className="section" aria-labelledby="section-title">
      <header className="section__header">
        <h1 id="section-title" className="section__title">
          {t("workflows.trial.screenTitle", { name: workflow.name })}
        </h1>
        <Button onClick={close}>{t("workflows.trial.back")}</Button>
      </header>
      <TrialPanel draft={draftOf(workflow)} workflowId={workflow.id} />
    </section>
  );
}
