//! krunch application crate — wires the pure domain, provider adapters, SQLite
//! store, and the orchestrator into a Tauri desktop app (PLAN §1).

mod commands;
mod credentials;
mod export;
mod gate;
mod provider_factory;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use krunch_engine::{AgentProvider, Engine, EngineConfig};
use krunch_store::Store;
use tauri::Manager;

use commands::AppState;
use provider_factory::KeychainProviderFactory;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // SQLite lives in the OS app-data dir (PLAN §7).
            let data_dir = app.path().app_data_dir().expect("resolve app data dir");
            std::fs::create_dir_all(&data_dir).ok();
            let store = Store::open(&data_dir.join("krunch.sqlite"))?;

            // Crash recovery: unfinished sessions → Interrupted (PLAN §7).
            {
                let store = store.clone();
                tauri::async_runtime::block_on(async move {
                    let _ = store.recover_on_startup().await;
                });
            }

            let provider: Arc<dyn AgentProvider> = Arc::new(KeychainProviderFactory);
            let engine = Arc::new(Engine::new(store.clone(), provider, EngineConfig::default()));

            app.manage(AppState {
                store,
                engine,
                pending: Arc::new(Mutex::new(HashMap::new())),
                runs: Arc::new(Mutex::new(HashMap::new())),
                event_capacity: 4096,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::core_version,
            commands::start_deliberation,
            commands::answer_questions,
            commands::abandon,
            commands::list_sessions,
            commands::get_session,
            commands::set_credential,
            commands::has_credential,
            commands::export_session,
            commands::save_session_dump,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
