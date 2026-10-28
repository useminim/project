//! Déclare les commandes de l'application dans le manifeste Tauri : chacune
//! reçoit une permission `allow-<commande>`, à accorder explicitement dans
//! `capabilities/default.json`. Une commande non déclarée ici est refusée.

/// Commandes appelables par l'interface (voir `src/commands/`).
const COMMANDS: &[&str] = &[
    "storage_status",
    "list_workflows",
    "get_workflow",
    "validate_workflow",
    "create_workflow",
    "update_workflow",
    "delete_workflow",
    "set_workflow_enabled",
    "reorder_workflows",
];

fn main() {
    let attributes = tauri_build::Attributes::new()
        .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS));
    if let Err(error) = tauri_build::try_build(attributes) {
        // Un script de build signale son échec par une panique.
        #[expect(clippy::panic, reason = "échec du script de build")]
        {
            panic!("{error:#}");
        }
    }
}
