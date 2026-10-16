import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { getStorageStatus } from "../api";
import type { StorageStatus } from "../bindings/StorageStatus";

/**
 * Signale un stockage local indisponible, une protection réduite ou une
 * ancienne base mise de côté au démarrage. Rien n'est affiché si tout va bien
 * ou si l'état ne peut pas être lu.
 */
export function StorageBanner() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<StorageStatus | null>(null);

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
      {status.reduced_protection && (
        <div className="banner banner--warning" role="status">
          {t("storage.reducedProtection")}
        </div>
      )}
    </>
  );
}
