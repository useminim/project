import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { getStorageStatus } from "../api";
import type { StorageStatus } from "../bindings/StorageStatus";
import { dismissReducedProtection, isReducedProtectionDismissed } from "../state/preferences";

/**
 * Signale un stockage local indisponible, une protection réduite ou une
 * ancienne base mise de côté au démarrage. Rien n'est affiché si tout va bien
 * ou si l'état ne peut pas être lu. L'avertissement « protection réduite »
 * peut être masqué ; il reste alors masqué aux lancements suivants.
 */
export function StorageBanner() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<StorageStatus | null>(null);
  const [reducedProtectionDismissed, setReducedProtectionDismissed] = useState(
    isReducedProtectionDismissed,
  );

  useEffect(() => {
    let active = true;
    getStorageStatus()
      .then((result) => {
        if (active) {
          setStatus(result);
        }
      })
      .catch(() => {
        // État illisible : l'interface reste utilisable, l'erreur est dans les logs Rust.
      });
    return () => {
      active = false;
    };
  }, []);

  if (!status) {
    return null;
  }
  if (!status.available) {
    return (
      <div className="banner banner--error" role="alert">
        {t("storage.unavailable")}
      </div>
    );
  }
  return (
    <>
      {status.recovered && (
        <div className="banner banner--warning" role="status">
          {t(`storage.recovered.${status.recovered}`)}
        </div>
      )}
      {status.reduced_protection && !reducedProtectionDismissed && (
        <div className="banner banner--warning banner--dismissible" role="status">
          <span className="banner__message">{t("storage.reducedProtection")}</span>
          <button
            type="button"
            className="banner__close"
            aria-label={t("storage.dismissReducedProtection")}
            onClick={() => {
              dismissReducedProtection();
              setReducedProtectionDismissed(true);
            }}
          >
            ×
          </button>
        </div>
      )}
    </>
  );
}
