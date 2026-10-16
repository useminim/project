//! Application desktop minim : mise en place du runtime Tauri.

pub mod commands;
pub mod diagnostics;
pub mod error;
pub mod logging;
pub mod storage;

use tauri::Manager;

/// Construit et exécute l'application Tauri jusqu'à la fermeture de la dernière fenêtre.
///
/// Les logs sont installés au démarrage dans le dossier de logs de
/// l'application ; leur garde est conservée dans l'état de Tauri. Le stockage
/// local chiffré est ensuite ouvert dans le dossier local de l'application
/// (non itinérant sous Windows).
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
            let info = diagnostics::DiagnosticInfo::current();
            tracing::info!(
                version = info.app_version,
                os = info.os,
                arch = info.arch,
                "démarrage de minim"
            );

            let storage = storage::open(&app.path().app_local_data_dir()?);
            tracing::info!(
                available = storage.status.available,
                reduced_protection = storage.status.reduced_protection,
                recovered = ?storage.status.recovered,
                "stockage local"
            );
            app.manage(storage);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![commands::storage_status])
        .run(tauri::generate_context!())
}
