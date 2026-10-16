// Préférences d'interface propres à ce poste, gardées dans le stockage local
// de la WebView. Rien de sensible : seulement des choix d'affichage. Un
// stockage indisponible (navigation privée, quota) revient aux valeurs par
// défaut.

const reducedProtectionDismissedKey = "minim.reducedProtectionDismissed";

/** L'utilisateur a masqué l'avertissement « protection réduite ». */
export function isReducedProtectionDismissed(): boolean {
  try {
    return localStorage.getItem(reducedProtectionDismissedKey) === "true";
  } catch {
    return false;
  }
}

/** Masque l'avertissement « protection réduite » aux prochains lancements. */
export function dismissReducedProtection(): void {
  try {
    localStorage.setItem(reducedProtectionDismissedKey, "true");
  } catch {
    // Stockage indisponible : l'avertissement reviendra au prochain lancement.
  }
}
