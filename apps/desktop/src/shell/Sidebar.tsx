import { useTranslation } from "react-i18next";
import { sections, useUiStore } from "../state/ui";

/** Barre latérale : une entrée par section, la section courante marquée `aria-current`. */
export function Sidebar() {
  const { t } = useTranslation();
  const current = useUiStore((state) => state.section);
  const navigate = useUiStore((state) => state.navigate);

  return (
    <nav className="sidebar" aria-label={t("navigation.label")}>
      <p className="sidebar__brand">{t("common.appName")}</p>
      <ul className="sidebar__list">
        {sections.map((section) => (
          <li key={section}>
            <button
              type="button"
              className="sidebar__link"
              aria-current={section === current ? "page" : undefined}
              onClick={() => navigate(section)}
            >
              {t(`navigation.${section}`)}
            </button>
          </li>
        ))}
      </ul>
    </nav>
  );
}
