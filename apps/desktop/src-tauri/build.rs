//! Script de build de l'application.
//!
//! - Déclare les commandes de l'application dans le manifeste Tauri : chacune
//!   reçoit une permission `allow-<commande>`, à accorder explicitement dans
//!   `capabilities/default.json`. Une commande non déclarée ici est refusée.
//! - Sous Windows (MSVC), embarque le manifeste d'application
//!   (`windows-app-manifest.xml`, Common Controls v6, celui de Tauri) dans
//!   **tous** les exécutables, binaires de test compris : `tauri-build` ne
//!   l'ajoute qu'au programme principal, et un test qui charge Tauri plante
//!   sans lui au démarrage (`STATUS_ENTRYPOINT_NOT_FOUND`).

use std::path::Path;

/// Commandes appelables par l'interface (voir `src/commands/`).
const COMMANDS: &[&str] = &[
    "storage_status",
    "list_workflows",
    "get_workflow",
    "validate_workflow",
    "preview_name",
    "create_workflow",
    "update_workflow",
    "delete_workflow",
    "set_workflow_enabled",
    "reorder_workflows",
    "preview_workflow_on_file",
    "run_workflow_on_file",
    "account_status",
    "sign_in",
    "sign_up",
    "sign_out",
    "request_password_reset",
];

const WINDOWS_MANIFEST: &str = "windows-app-manifest.xml";

fn main() {
    let mut attributes = tauri_build::Attributes::new()
        .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS));

    let target = |key: &str| std::env::var(key).unwrap_or_default();
    if target("CARGO_CFG_TARGET_OS") == "windows" && target("CARGO_CFG_TARGET_ENV") == "msvc" {
        attributes = attributes
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
        let manifest = Path::new(&target("CARGO_MANIFEST_DIR")).join(WINDOWS_MANIFEST);
        println!("cargo:rerun-if-changed={WINDOWS_MANIFEST}");
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }

    if let Err(error) = tauri_build::try_build(attributes) {
        // Un script de build signale son échec par une panique.
        #[expect(clippy::panic, reason = "échec du script de build")]
        {
            panic!("{error:#}");
        }
    }
}
