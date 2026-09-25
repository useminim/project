//! minim desktop application: Tauri runtime setup.

/// Builds and runs the Tauri application until the last window is closed.
///
/// # Errors
///
/// Returns an error if the Tauri runtime fails to start.
pub fn run() -> tauri::Result<()> {
    tauri::Builder::default().run(tauri::generate_context!())
}
