import { open } from "@tauri-apps/plugin-dialog";

/**
 * Ouvre la boîte de dialogue système de choix d'un dossier. Renvoie le chemin
 * choisi, ou `null` si l'utilisateur annule.
 */
export async function pickFolder(title: string, defaultPath?: string): Promise<string | null> {
  const chosen = await open({ directory: true, multiple: false, title, defaultPath });
  return typeof chosen === "string" ? chosen : null;
}
