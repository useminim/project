import { useTranslation } from "react-i18next";
import { EmptyState } from "../components/EmptyState";
import type { Section } from "../state/ui";

/** Contenu d'une section. Les écrans réels remplaceront ces états vides. */
export function SectionView({ section }: { section: Section }) {
  const { t } = useTranslation();

  return (
    <section className="section" aria-labelledby="section-title">
      <h1 id="section-title" className="section__title">
        {t(`${section}.title`)}
      </h1>
      <EmptyState
        title={t(`${section}.empty.title`)}
        description={t(`${section}.empty.description`)}
      />
    </section>
  );
}
