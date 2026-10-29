import { useTranslation } from "react-i18next";
import { AccountPanel } from "./AccountPanel";

/** Section « Paramètres » : pour l'instant, le compte. */
export function SettingsScreen() {
  const { t } = useTranslation();

  return (
    <section className="section" aria-labelledby="section-title">
      <h1 id="section-title" className="section__title">
        {t("settings.title")}
      </h1>
      <AccountPanel />
    </section>
  );
}
