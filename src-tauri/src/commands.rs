//! Tauri command surface (PLAN §1). Commands are thin: they validate, talk to the
//! store/engine, and forward engine events to the webview. Secrets never cross to
//! the webview; the webview only passes a `credential_ref`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use krunch_core::config::SessionConfig;
use krunch_core::ids::SessionId;
use krunch_core::state::SessionState;
use krunch_engine::{Engine, UserGate};
use krunch_store::{PresetRow, SessionSummary, Store};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::credentials;
use crate::gate::{self, PendingGates, TauriUserGate};

/// Shared application state.
pub struct AppState {
    pub store: Store,
    pub engine: Arc<Engine>,
    pub pending: PendingGates,
    pub runs: Arc<Mutex<HashMap<SessionId, CancellationToken>>>,
    pub event_capacity: usize,
}

/// Event channel name the webview subscribes to.
pub const EVENT_CHANNEL: &str = "krunch://event";

#[derive(Debug, Serialize)]
pub struct StartDto {
    pub session_id: SessionId,
    pub created: bool,
}

#[derive(Debug, Serialize)]
pub struct SessionDto {
    pub id: SessionId,
    pub state: SessionState,
    pub max_rounds: u32,
    pub problem: String,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<SessionSummary> for SessionDto {
    fn from(s: SessionSummary) -> Self {
        Self {
            id: s.id,
            state: s.state,
            max_rounds: s.max_rounds,
            problem: s.problem,
            created_at: s.created_at,
            updated_at: s.updated_at,
        }
    }
}

fn parse_session(id: &str) -> Result<SessionId, String> {
    Uuid::parse_str(id).map(SessionId).map_err(|e| format!("bad session id: {e}"))
}

/// Validate a config, create (idempotently) the session, and spawn the run.
#[tauri::command]
pub async fn start_deliberation(
    app: AppHandle,
    state: State<'_, AppState>,
    idempotency_key: String,
    config: SessionConfig,
) -> Result<StartDto, String> {
    config
        .validate()
        .map_err(|errs| errs.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; "))?;

    // Extract everything we need before awaiting (State can't cross .await).
    let store = state.store.clone();
    let engine = state.engine.clone();
    let pending = state.pending.clone();
    let runs = state.runs.clone();
    let capacity = state.event_capacity;

    let created = store
        .create_session(idempotency_key, config.clone())
        .await
        .map_err(|e| e.to_string())?;
    let session = created.session_id;

    if created.created {
        let cancel = CancellationToken::new();
        runs.lock().unwrap().insert(session, cancel.clone());

        let (tx, mut rx) = mpsc::channel(capacity);
        let app2 = app.clone();
        tokio::spawn(async move {
            while let Some(ev) = rx.recv().await {
                let _ = app2.emit(EVENT_CHANNEL, ev);
            }
        });

        let gate: Arc<dyn UserGate> =
            Arc::new(TauriUserGate { session, pending, cancel: cancel.clone() });
        tokio::spawn(async move {
            let _ = engine.run(session, config, gate, tx, cancel).await;
            runs.lock().unwrap().remove(&session);
        });
    }

    Ok(StartDto { session_id: session, created: created.created })
}

/// Deliver the user's answers to a paused deliberation.
#[tauri::command]
pub async fn answer_questions(
    state: State<'_, AppState>,
    session_id: String,
    answers: Vec<(String, String)>,
) -> Result<bool, String> {
    let session = parse_session(&session_id)?;
    Ok(gate::answer(&state.pending, session, answers))
}

/// Abandon a running deliberation (cancels tasks + unblocks any pending gate).
#[tauri::command]
pub async fn abandon(state: State<'_, AppState>, session_id: String) -> Result<(), String> {
    let session = parse_session(&session_id)?;
    if let Some(cancel) = state.runs.lock().unwrap().remove(&session) {
        cancel.cancel();
    }
    Ok(())
}

/// List all sessions, newest first.
#[tauri::command]
pub async fn list_sessions(state: State<'_, AppState>) -> Result<Vec<SessionDto>, String> {
    let store = state.store.clone();
    let rows = store.list_sessions().await.map_err(|e| e.to_string())?;
    Ok(rows.into_iter().map(SessionDto::from).collect())
}

/// Fetch one session summary.
#[tauri::command]
pub async fn get_session(state: State<'_, AppState>, session_id: String) -> Result<SessionDto, String> {
    let session = parse_session(&session_id)?;
    let store = state.store.clone();
    store.get_session(session).await.map(SessionDto::from).map_err(|e| e.to_string())
}

/// Store a provider key in the OS keychain under `credential_ref`.
#[tauri::command]
pub async fn set_credential(credential_ref: String, secret: String) -> Result<(), String> {
    credentials::store(&credential_ref, &secret)
}

/// Whether a key exists for `credential_ref` (never reveals it).
#[tauri::command]
pub async fn has_credential(credential_ref: String) -> Result<bool, String> {
    Ok(credentials::exists(&credential_ref))
}

/// Export a finished (or in-progress) session as self-contained Markdown.
#[tauri::command]
pub async fn export_session(state: State<'_, AppState>, session_id: String) -> Result<String, String> {
    let session = parse_session(&session_id)?;
    let store = state.store.clone();
    crate::export::export_markdown(&store, session).await
}

/// Save the session dump to the user's Downloads folder and reveal it in the
/// file manager. Returns the written path. (A blob-anchor download does not
/// work inside the WKWebView, so the file is written natively.)
#[tauri::command]
pub async fn save_session_dump(state: State<'_, AppState>, session_id: String) -> Result<String, String> {
    let session = parse_session(&session_id)?;
    let store = state.store.clone();
    let markdown = crate::export::export_markdown(&store, session).await?;

    let dir = dirs::download_dir().ok_or("could not locate the Downloads folder")?;
    let short = session_id.get(..8).unwrap_or(&session_id);
    let path = dir.join(format!("krunch-session-{short}.md"));
    std::fs::write(&path, markdown).map_err(|e| format!("write {}: {e}", path.display()))?;
    // Best-effort reveal; the save itself already succeeded.
    let _ = tauri_plugin_opener::reveal_item_in_dir(&path);
    Ok(path.display().to_string())
}

/// Health check retained from M1.
#[tauri::command]
pub fn core_version() -> String {
    krunch_core::version().to_string()
}

/// Read a raw setting value (opaque JSON) by key.
#[tauri::command]
pub async fn get_setting(state: State<'_, AppState>, key: String) -> Result<Option<String>, String> {
    state.store.clone().get_setting(key).await.map_err(|e| e.to_string())
}

/// Upsert a setting value.
#[tauri::command]
pub async fn set_setting(state: State<'_, AppState>, key: String, value: String) -> Result<(), String> {
    state.store.clone().set_setting(key, value).await.map_err(|e| e.to_string())
}

/// List saved panel presets, newest first.
#[tauri::command]
pub async fn list_presets(state: State<'_, AppState>) -> Result<Vec<PresetRow>, String> {
    state.store.clone().list_presets().await.map_err(|e| e.to_string())
}

/// Save (upsert on name) a panel preset; returns the row id.
#[tauri::command]
pub async fn save_preset(
    state: State<'_, AppState>,
    name: String,
    config_json: String,
) -> Result<String, String> {
    state.store.clone().save_preset(name, config_json).await.map_err(|e| e.to_string())
}

/// Delete a preset by id.
#[tauri::command]
pub async fn delete_preset(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.store.clone().delete_preset(id).await.map_err(|e| e.to_string())
}
