import { useTranslation } from "react-i18next";
import { issuesAt } from "./draft";
import { issueMessages, type WizardIssue } from "./WizardScreen";

/** Problèmes situés à `path`, affichés sous l'élément concerné. */
export function FieldErrors({
  issues,
  path,
  id,
}: {
  issues: readonly WizardIssue[];
  path: string;
  id?: string;
}) {
  const { t } = useTranslation();
  const messages = issueMessages(t, issuesAt(issues, path));
  if (messages.length === 0) {
    return null;
  }
  return (
    <ul id={id} className="field__error wizard__errors">
      {messages.map((message) => (
        <li key={message}>{message}</li>
      ))}
    </ul>
  );
}
