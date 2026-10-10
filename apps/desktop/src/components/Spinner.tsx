import { useTranslation } from "react-i18next";

/** Indicateur de chargement, annoncé aux lecteurs d'écran. */
export function Spinner() {
  const { t } = useTranslation();

  return (
    <output className="spinner">
      <span className="spinner__circle" aria-hidden="true" />
      <span className="visually-hidden">{t("common.loading")}</span>
    </output>
  );
}
