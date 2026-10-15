// Notifications (Radix Toast) : annoncées aux lecteurs d'écran, fermées au
// clavier ou après quelques secondes. On en affiche avec
// `useUiStore.getState().showToast(kind, clé)`.

import * as Toast from "@radix-ui/react-toast";
import { useTranslation } from "react-i18next";
import { useUiStore } from "../state/ui";

const durationMs = 6000;

export function Toaster() {
  const { t } = useTranslation();
  const toasts = useUiStore((state) => state.toasts);
  const dismissToast = useUiStore((state) => state.dismissToast);

  return (
    <Toast.Provider duration={durationMs} label={t("common.notifications")}>
      {toasts.map((toast) => (
        <Toast.Root
          key={toast.id}
          className={`toast toast--${toast.kind}`}
          type={toast.kind === "error" ? "foreground" : "background"}
          onOpenChange={(open) => {
            if (!open) {
              dismissToast(toast.id);
            }
          }}
        >
          <Toast.Description className="toast__message">{t(toast.messageKey)}</Toast.Description>
          <Toast.Close className="toast__close" aria-label={t("common.close")}>
            ×
          </Toast.Close>
        </Toast.Root>
      ))}
      <Toast.Viewport className="toast-viewport" />
    </Toast.Provider>
  );
}
