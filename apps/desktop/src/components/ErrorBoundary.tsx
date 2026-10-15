// Dernier rempart : une erreur de rendu affiche un message traduit et propose
// de recharger l'interface au lieu de laisser une fenêtre blanche. Le détail
// de l'erreur n'est pas affiché, il pourrait contenir des données d'un
// document. Sa journalisation côté Rust viendra avec une commande dédiée.

import { Component, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "./Button";

interface ErrorBoundaryProps {
  children: ReactNode;
  /** Remplace le rechargement de la page, pour les tests. */
  onReload?: () => void;
}

interface ErrorBoundaryState {
  failed: boolean;
}

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  override state: ErrorBoundaryState = { failed: false };

  static getDerivedStateFromError(): ErrorBoundaryState {
    return { failed: true };
  }

  override render() {
    if (this.state.failed) {
      return <CrashScreen onReload={this.props.onReload ?? (() => window.location.reload())} />;
    }
    return this.props.children;
  }
}

function CrashScreen({ onReload }: { onReload: () => void }) {
  const { t } = useTranslation();

  return (
    <div className="crash" role="alert">
      <h1 className="crash__title">{t("common.crash.title")}</h1>
      <p className="crash__description">{t("common.crash.description")}</p>
      <Button variant="primary" onClick={onReload}>
        {t("common.crash.reload")}
      </Button>
    </div>
  );
}
