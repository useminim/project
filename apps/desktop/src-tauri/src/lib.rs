//! Application desktop minim : mise en place du runtime Tauri.

pub mod logging;

use tauri::Manager;

/// Construit et exécute l'application Tauri jusqu'à la fermeture de la dernière fenêtre.
///
/// Les logs sont installés au démarrage dans le dossier de logs de
/// l'application ; leur garde est conservée dans l'état de Tauri.
///
/// # Erreurs
///
/// Renvoie une erreur si le runtime Tauri ne parvient pas à démarrer, ou si
/// les logs ne peuvent pas être installés.
pub fn run() -> tauri::Result<()> {
    tauri::Builder::default()
        .setup(|app| {
            let log_dir = app.path().app_log_dir()?;
            let guard = logging::init(&log_dir)?;
            app.manage(guard);
            tracing::info!(version = env!("CARGO_PKG_VERSION"), "démarrage de minim");
            Ok(())
        })
        .run(tauri::generate_context!())
}
