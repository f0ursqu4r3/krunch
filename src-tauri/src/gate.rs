//! The Tauri user gate (PLAN §5). When the mediator pauses, the engine calls
//! `ask`, which parks on a oneshot until `answer_questions` (or `abandon`) resolves
//! it. A per-session registry maps a waiting session to its answer channel.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use krunch_core::ids::SessionId;
use krunch_engine::{GateResponse, UserGate};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

/// Shared registry of sessions currently awaiting a user answer.
pub type PendingGates = Arc<Mutex<HashMap<SessionId, oneshot::Sender<GateResponse>>>>;

/// Per-session gate handed to the engine run.
pub struct TauriUserGate {
    pub session: SessionId,
    pub pending: PendingGates,
    pub cancel: CancellationToken,
}

#[async_trait]
impl UserGate for TauriUserGate {
    async fn ask(&self, _round_index: u32, _questions: Vec<String>) -> GateResponse {
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(self.session, tx);

        let response = tokio::select! {
            _ = self.cancel.cancelled() => GateResponse::Abandon,
            r = rx => r.unwrap_or(GateResponse::Abandon),
        };
        // Clear any stale entry (e.g. resolved via cancel).
        self.pending.lock().unwrap().remove(&self.session);
        response
    }
}

/// Fulfil a waiting gate with user answers. Returns false if nothing was waiting.
pub fn answer(pending: &PendingGates, session: SessionId, answers: Vec<(String, String)>) -> bool {
    if let Some(tx) = pending.lock().unwrap().remove(&session) {
        tx.send(GateResponse::Answers(answers)).is_ok()
    } else {
        false
    }
}
