//! krunch application crate.
//!
//! Wires the pure `krunch-core` domain to the outside world: provider adapters
//! (M3), SQLite persistence (M4), the orchestrator + Tauri commands (M5). For now
//! this is the M1 skeleton exposing a single health-check command.

#[tauri::command]
fn core_version() -> String {
    krunch_core::version().to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![core_version])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
