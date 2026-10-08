//! Application desktop minim : mise en place du runtime Tauri.

/// Construit et exécute l'application Tauri jusqu'à la fermeture de la dernière fenêtre.
///
/// # Erreurs
///
/// Renvoie une erreur si le runtime Tauri ne parvient pas à démarrer.
pub fn run() -> tauri::Result<()> {
    tauri::Builder::default().run(tauri::generate_context!())
}
