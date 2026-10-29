//! Application desktop minim : mise en place du runtime Tauri.

pub mod commands;
pub mod diagnostics;
pub mod error;
pub mod logging;
pub mod pdf;
pub mod storage;

use std::sync::Arc;

use minim_core::ports::{SystemClock, UlidGenerator};
use minim_core::{TrialService, WorkflowService};
use tauri::Manager;

use crate::commands::trials::Trials;
use crate::commands::workflows::Workflows;

/// Construit et exécute l'application Tauri jusqu'à la fermeture de la dernière fenêtre.
///
/// Les logs sont installés au démarrage dans le dossier de logs de
/// l'application ; leur garde est conservée dans l'état de Tauri. Le stockage
/// local chiffré est ensuite ouvert dans le dossier local de l'application
/// (non itinérant sous Windows), et le processus PDFium préparé.
///
/// Lancé avec l'argument `--pdf-worker`, l'exécutable devient le processus
/// PDFium (D-36) : il traite les demandes de l'application et s'arrête avec
/// elle, sans ouvrir de fenêtre.
///
/// # Erreurs
///
/// Renvoie une erreur si le runtime Tauri ne parvient pas à démarrer, ou si
/// les logs ne peuvent pas être installés.
pub fn run() -> tauri::Result<()> {
    if let Some(code) = minim_extract::serve_if_requested(std::env::args_os()) {
        std::process::exit(code);
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
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
            let worker = pdf::prepare(&app.path().resource_dir()?);
            let converter = pdf::converter(worker.as_ref());
            if let Some(worker) = worker {
                app.manage(worker);
            }

            let clock = Arc::new(SystemClock);
            let ids = Arc::new(UlidGenerator);
            let workflows = storage.repositories.as_ref().map(|repositories| {
                WorkflowService::new(
                    Arc::clone(&repositories.workflows),
                    clock.clone(),
                    ids.clone(),
                )
            });
            let trials = storage.repositories.as_ref().zip(workflows.clone()).map(
                |(repositories, workflows)| {
                    TrialService::new(
                        workflows,
                        Arc::clone(&repositories.execution_logs),
                        converter,
                        clock,
                        ids,
                        Arc::new(|date: chrono::DateTime<chrono::Utc>| {
                            date.with_timezone(&chrono::Local).date_naive()
                        }),
                    )
                },
            );
            app.manage(Workflows(workflows));
            app.manage(Trials(trials));
            app.manage(storage);
            Ok(())
        })
        .invoke_handler(commands::handler())
        .run(tauri::generate_context!())
}
