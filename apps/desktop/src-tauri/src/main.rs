// Empêche l'ouverture d'une fenêtre de console supplémentaire sous Windows en release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> tauri::Result<()> {
    minim_desktop_lib::run()
}
