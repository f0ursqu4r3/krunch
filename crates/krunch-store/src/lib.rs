//! krunch-store — SQLite persistence (PLAN §7).
//!
//! A **single dedicated writer thread** owns the one `Connection`; every operation
//! is a closure shipped to it over a channel, so there is no lock contention and
//! all access is serialized. Streamed tokens are appended as **chunks** under an
//! **attempt** whose lifecycle is provisional → accepted | discarded — a retry
//! starts a new attempt and discards the old one, so partial output never mixes
//! (the generation fence). Crash recovery marks unfinished sessions `Interrupted`.

pub mod schema;

use std::path::Path;
use std::sync::mpsc;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};
use tokio::sync::oneshot;

use krunch_core::config::{SeatConfig, SessionConfig};
use krunch_core::ids::{AttemptId, RoundId, SeatId, SessionId};
use krunch_core::schema::{MediatorRuling, Stance};
use krunch_core::state::SessionState;

/// Errors surfaced by the store.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("the writer thread is gone")]
    WriterGone,
    #[error("generation fence: attempt {attempt} is {status}, not provisional")]
    FenceViolation { attempt: AttemptId, status: String },
    #[error("not found: {0}")]
    NotFound(String),
    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),
}

type StoreResult<T> = Result<T, StoreError>;
type Job = Box<dyn FnOnce(&mut Connection) + Send>;

/// Handle to the persistence layer. Cheap to clone (shares the writer channel).
#[derive(Clone)]
pub struct Store {
    tx: mpsc::Sender<Job>,
}

/// Outcome of an idempotent `create_session`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateOutcome {
    pub session_id: SessionId,
    /// False when the idempotency key already existed (the existing session is returned).
    pub created: bool,
}

/// A read-model summary of a session row.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionSummary {
    pub id: SessionId,
    pub state: SessionState,
    pub max_rounds: u32,
    pub problem: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// A saved panel preset (read model + wire type).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct PresetRow {
    pub id: String,
    pub name: String,
    pub config_json: String,
    pub updated_at: i64,
}

/// The kind of round (PLAN §6 — finalization reuses the round/attempt spine).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundKind {
    Deliberation,
    Finalization,
}

impl RoundKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Deliberation => "deliberation",
            Self::Finalization => "finalization",
        }
    }
}

/// A diagnostic error record (PLAN §7).
#[derive(Debug, Clone, Default)]
pub struct ErrorRecord {
    pub round_id: Option<RoundId>,
    pub seat_id: Option<SeatId>,
    pub attempt_id: Option<AttemptId>,
    pub kind: String,
    pub http_status: Option<u16>,
    pub retry_count: u32,
    pub deadline_hit: bool,
    pub provider_request_id: Option<String>,
    pub detail: String,
}

fn now_millis() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64
}

/// Serialize an enum that renders as a JSON string to that bare string.
fn to_tag<T: Serialize>(v: &T) -> String {
    serde_json::to_value(v)
        .ok()
        .and_then(|x| x.as_str().map(str::to_string))
        .expect("enum serializes to a string")
}

/// Inverse of [`to_tag`].
fn from_tag<T: DeserializeOwned>(s: &str) -> StoreResult<T> {
    Ok(serde_json::from_value(serde_json::Value::String(s.to_string()))?)
}

impl Store {
    /// Open (creating if needed) the database at `path`, run migrations, enable
    /// WAL + foreign keys, restrict file permissions to the owner, and spawn the
    /// writer thread.
    pub fn open(path: &Path) -> StoreResult<Store> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        schema::migrate(&conn)?;
        set_owner_only(path);

        let (tx, rx) = mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name("krunch-store-writer".into())
            .spawn(move || {
                let mut conn = conn;
                while let Ok(job) = rx.recv() {
                    job(&mut conn);
                }
            })
            .expect("spawn writer thread");
        Ok(Store { tx })
    }

    /// Ship a closure to the writer thread and await its typed result.
    async fn run<T, F>(&self, f: F) -> StoreResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> StoreResult<T> + Send + 'static,
    {
        let (reply, rx) = oneshot::channel();
        let job: Job = Box::new(move |conn| {
            let _ = reply.send(f(conn));
        });
        self.tx.send(job).map_err(|_| StoreError::WriterGone)?;
        rx.await.map_err(|_| StoreError::WriterGone)?
    }

    /// Idempotently create the `Starting` session row + audit snapshot (PLAN §1/§3).
    /// A duplicate `idempotency_key` returns the existing session, `created=false`.
    pub async fn create_session(
        &self,
        idempotency_key: String,
        cfg: SessionConfig,
    ) -> StoreResult<CreateOutcome> {
        self.run(move |conn| {
            let tx = conn.transaction()?;
            let new_id = SessionId::new();
            let now = now_millis();
            let changed = tx.execute(
                "INSERT INTO sessions
                    (id, idempotency_key, state, mode, max_rounds, quorum_fraction,
                     confidence_floor, problem, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?9)
                 ON CONFLICT(idempotency_key) DO NOTHING",
                params![
                    new_id.to_string(),
                    idempotency_key,
                    to_tag(&SessionState::Starting),
                    to_tag(&cfg.mode),
                    cfg.max_rounds,
                    cfg.guard.quorum_fraction,
                    cfg.guard.confidence_floor,
                    cfg.problem,
                    now,
                ],
            )?;
            let created = changed > 0;
            let id_str: String = tx.query_row(
                "SELECT id FROM sessions WHERE idempotency_key = ?1",
                params![idempotency_key],
                |r| r.get(0),
            )?;
            let session_id = SessionId(id_str.parse().map_err(|_| {
                StoreError::NotFound(format!("bad session uuid {id_str}"))
            })?);

            if created {
                for seat in &cfg.seats {
                    insert_seat(&tx, session_id, seat)?;
                }
            }
            tx.commit()?;
            Ok(CreateOutcome { session_id, created })
        })
        .await
    }

    /// Transition a session's persisted state.
    pub async fn set_state(&self, session: SessionId, state: SessionState) -> StoreResult<()> {
        self.run(move |conn| {
            let n = conn.execute(
                "UPDATE sessions SET state = ?2, updated_at = ?3 WHERE id = ?1",
                params![session.to_string(), to_tag(&state), now_millis()],
            )?;
            if n == 0 {
                return Err(StoreError::NotFound(format!("session {session}")));
            }
            Ok(())
        })
        .await
    }

    /// Insert a fresh `running` round.
    pub async fn begin_round(
        &self,
        session: SessionId,
        index_no: u32,
        kind: RoundKind,
        focus: Option<String>,
    ) -> StoreResult<RoundId> {
        self.run(move |conn| {
            let id = RoundId::new();
            conn.execute(
                "INSERT INTO rounds (id, session_id, index_no, kind, status, focus, created_at)
                 VALUES (?1,?2,?3,?4,'running',?5,?6)",
                params![
                    id.to_string(),
                    session.to_string(),
                    index_no,
                    kind.as_str(),
                    focus,
                    now_millis()
                ],
            )?;
            Ok(id)
        })
        .await
    }

    /// Start a new provisional attempt for `(round, seat)`, discarding any prior
    /// provisional attempt for that pair (the generation fence advances).
    pub async fn start_attempt(&self, round: RoundId, seat: SeatId) -> StoreResult<AttemptId> {
        self.run(move |conn| {
            let tx = conn.transaction()?;
            let next_no: i64 = tx.query_row(
                "SELECT COALESCE(MAX(attempt_no) + 1, 0) FROM attempts
                 WHERE round_id = ?1 AND seat_id = ?2",
                params![round.to_string(), seat.to_string()],
                |r| r.get(0),
            )?;
            tx.execute(
                "UPDATE attempts SET status = 'discarded'
                 WHERE round_id = ?1 AND seat_id = ?2 AND status = 'provisional'",
                params![round.to_string(), seat.to_string()],
            )?;
            let id = AttemptId::new();
            tx.execute(
                "INSERT INTO attempts (id, round_id, seat_id, attempt_no, status, created_at)
                 VALUES (?1,?2,?3,?4,'provisional',?5)",
                params![id.to_string(), round.to_string(), seat.to_string(), next_no, now_millis()],
            )?;
            tx.commit()?;
            Ok(id)
        })
        .await
    }

    /// Append a batch of streamed chunks to a *provisional* attempt. Appending to
    /// a discarded/accepted attempt is a [`StoreError::FenceViolation`].
    pub async fn append_chunks(&self, attempt: AttemptId, chunks: Vec<String>) -> StoreResult<()> {
        self.run(move |conn| {
            let status: Option<String> = conn
                .query_row(
                    "SELECT status FROM attempts WHERE id = ?1",
                    params![attempt.to_string()],
                    |r| r.get(0),
                )
                .optional()?;
            let status = status.ok_or_else(|| StoreError::NotFound(format!("attempt {attempt}")))?;
            if status != "provisional" {
                return Err(StoreError::FenceViolation { attempt, status });
            }
            let tx = conn.transaction()?;
            let base_seq: i64 = tx.query_row(
                "SELECT COALESCE(MAX(seq) + 1, 0) FROM chunks WHERE attempt_id = ?1",
                params![attempt.to_string()],
                |r| r.get(0),
            )?;
            for (i, c) in chunks.into_iter().enumerate() {
                tx.execute(
                    "INSERT INTO chunks (attempt_id, seq, content) VALUES (?1,?2,?3)",
                    params![attempt.to_string(), base_seq + i as i64, c],
                )?;
            }
            tx.commit()?;
            Ok(())
        })
        .await
    }

    /// Accept an attempt as the seat's round output; discard its siblings.
    pub async fn accept_attempt(&self, attempt: AttemptId) -> StoreResult<()> {
        self.run(move |conn| {
            let tx = conn.transaction()?;
            let (round_id, seat_id): (String, String) = tx
                .query_row(
                    "SELECT round_id, seat_id FROM attempts WHERE id = ?1",
                    params![attempt.to_string()],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?
                .ok_or_else(|| StoreError::NotFound(format!("attempt {attempt}")))?;
            tx.execute(
                "UPDATE attempts SET status = 'discarded'
                 WHERE round_id = ?1 AND seat_id = ?2 AND id != ?3",
                params![round_id, seat_id, attempt.to_string()],
            )?;
            tx.execute(
                "UPDATE attempts SET status = 'accepted' WHERE id = ?1",
                params![attempt.to_string()],
            )?;
            tx.commit()?;
            Ok(())
        })
        .await
    }

    /// Mark an attempt discarded (e.g. it failed and won't be retried).
    pub async fn discard_attempt(&self, attempt: AttemptId) -> StoreResult<()> {
        self.run(move |conn| {
            conn.execute(
                "UPDATE attempts SET status = 'discarded' WHERE id = ?1",
                params![attempt.to_string()],
            )?;
            Ok(())
        })
        .await
    }

    /// Persist a panelist's validated stance for a round.
    pub async fn record_stance(
        &self,
        round: RoundId,
        seat: SeatId,
        stance: Stance,
    ) -> StoreResult<()> {
        self.run(move |conn| {
            conn.execute(
                "INSERT OR REPLACE INTO stances
                    (round_id, seat_id, stance, confidence, agree_with_json, open_questions_json)
                 VALUES (?1,?2,?3,?4,?5,?6)",
                params![
                    round.to_string(),
                    seat.to_string(),
                    stance.stance,
                    stance.confidence,
                    serde_json::to_string(&stance.agree_with)?,
                    serde_json::to_string(&stance.open_questions)?,
                ],
            )?;
            Ok(())
        })
        .await
    }

    /// Persist the mediator's ruling (incl. the capped `summary` ledger field).
    pub async fn record_ruling(&self, round: RoundId, ruling: MediatorRuling) -> StoreResult<()> {
        self.run(move |conn| {
            conn.execute(
                "INSERT OR REPLACE INTO rulings
                    (round_id, ruling, request_user_input, next_focus,
                     questions_json, assumptions_json, summary)
                 VALUES (?1,?2,?3,?4,?5,?6,?7)",
                params![
                    round.to_string(),
                    to_tag(&ruling.ruling),
                    ruling.request_user_input as i64,
                    ruling.next_focus,
                    serde_json::to_string(&ruling.questions_for_user)?,
                    serde_json::to_string(&ruling.assumptions)?,
                    ruling.summary,
                ],
            )?;
            Ok(())
        })
        .await
    }

    /// Atomically finalize a round (PLAN §3i).
    pub async fn finalize_round(&self, round: RoundId) -> StoreResult<()> {
        self.run(move |conn| {
            let n = conn.execute(
                "UPDATE rounds SET status = 'finalized' WHERE id = ?1 AND status = 'running'",
                params![round.to_string()],
            )?;
            if n == 0 {
                return Err(StoreError::NotFound(format!("running round {round}")));
            }
            Ok(())
        })
        .await
    }

    /// Record a resolved user Q&A exchange.
    pub async fn record_user_qa(
        &self,
        session: SessionId,
        round_index: u32,
        question: String,
        answer: String,
    ) -> StoreResult<()> {
        self.run(move |conn| {
            conn.execute(
                "INSERT INTO user_qa (session_id, round_index, question, answer, created_at)
                 VALUES (?1,?2,?3,?4,?5)",
                params![session.to_string(), round_index, question, answer, now_millis()],
            )?;
            Ok(())
        })
        .await
    }

    /// Persist a diagnostic error record.
    pub async fn record_error(&self, session: SessionId, rec: ErrorRecord) -> StoreResult<()> {
        self.run(move |conn| {
            conn.execute(
                "INSERT INTO error_records
                    (session_id, round_id, seat_id, attempt_id, kind, http_status,
                     retry_count, deadline_hit, provider_request_id, detail, created_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                params![
                    session.to_string(),
                    rec.round_id.map(|r| r.to_string()),
                    rec.seat_id.map(|s| s.to_string()),
                    rec.attempt_id.map(|a| a.to_string()),
                    rec.kind,
                    rec.http_status,
                    rec.retry_count,
                    rec.deadline_hit as i64,
                    rec.provider_request_id,
                    rec.detail,
                    now_millis(),
                ],
            )?;
            Ok(())
        })
        .await
    }

    /// Startup crash recovery (PLAN §7): mark every non-terminal session
    /// `Interrupted`, abandon `running` rounds, and discard their provisional
    /// attempts. Returns the number of sessions interrupted.
    pub async fn recover_on_startup(&self) -> StoreResult<usize> {
        self.run(move |conn| {
            let tx = conn.transaction()?;
            let n = tx.execute(
                "UPDATE sessions SET state = 'interrupted', updated_at = ?1
                 WHERE state NOT IN
                    ('converged','deadlocked','halted','mediator_error','interrupted','abandoned')",
                params![now_millis()],
            )?;
            tx.execute(
                "UPDATE attempts SET status = 'discarded'
                 WHERE status = 'provisional'
                   AND round_id IN (SELECT id FROM rounds WHERE status = 'running')",
                [],
            )?;
            tx.execute("UPDATE rounds SET status = 'abandoned' WHERE status = 'running'", [])?;
            tx.commit()?;
            Ok(n)
        })
        .await
    }

    /// Read a raw setting value (opaque JSON string) by key.
    pub async fn get_setting(&self, key: String) -> StoreResult<Option<String>> {
        self.run(move |conn| {
            Ok(conn
                .query_row(
                    "SELECT value FROM app_settings WHERE key = ?1",
                    params![key],
                    |r| r.get(0),
                )
                .optional()?)
        })
        .await
    }

    /// Upsert a setting value.
    pub async fn set_setting(&self, key: String, value: String) -> StoreResult<()> {
        self.run(move |conn| {
            conn.execute(
                "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )?;
            Ok(())
        })
        .await
    }

    /// Delete a setting.
    pub async fn delete_setting(&self, key: String) -> StoreResult<()> {
        self.run(move |conn| {
            conn.execute("DELETE FROM app_settings WHERE key = ?1", params![key])?;
            Ok(())
        })
        .await
    }

    /// Save (upsert on `name`) a panel preset; returns the row id.
    pub async fn save_preset(&self, name: String, config_json: String) -> StoreResult<String> {
        self.run(move |conn| {
            let now = now_millis();
            let new_id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO panel_presets (id, name, config_json, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?4)
                 ON CONFLICT(name) DO UPDATE SET
                     config_json = excluded.config_json,
                     updated_at  = excluded.updated_at",
                params![new_id, name, config_json, now],
            )?;
            let id: String = conn.query_row(
                "SELECT id FROM panel_presets WHERE name = ?1",
                params![name],
                |r| r.get(0),
            )?;
            Ok(id)
        })
        .await
    }

    /// List presets, most-recently-updated first.
    pub async fn list_presets(&self) -> StoreResult<Vec<PresetRow>> {
        self.run(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, config_json, updated_at
                 FROM panel_presets ORDER BY updated_at DESC",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok(PresetRow {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    config_json: r.get(2)?,
                    updated_at: r.get(3)?,
                })
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
        .await
    }

    /// Delete a preset by id.
    pub async fn delete_preset(&self, id: String) -> StoreResult<()> {
        self.run(move |conn| {
            conn.execute("DELETE FROM panel_presets WHERE id = ?1", params![id])?;
            Ok(())
        })
        .await
    }

    /// Persist the per-session pre-resolution editing snapshot (opaque JSON).
    pub async fn set_session_setup(&self, session: SessionId, setup_json: String) -> StoreResult<()> {
        self.run(move |conn| {
            conn.execute(
                "INSERT INTO session_setup (session_id, setup_json) VALUES (?1, ?2)
                 ON CONFLICT(session_id) DO UPDATE SET setup_json = excluded.setup_json",
                params![session.to_string(), setup_json],
            )?;
            Ok(())
        })
        .await
    }

    /// Read the editing snapshot for a session, if captured.
    pub async fn get_session_setup(&self, session: SessionId) -> StoreResult<Option<String>> {
        self.run(move |conn| {
            Ok(conn
                .query_row(
                    "SELECT setup_json FROM session_setup WHERE session_id = ?1",
                    params![session.to_string()],
                    |r| r.get(0),
                )
                .optional()?)
        })
        .await
    }

    // --- reads ---

    /// Fetch a session summary.
    pub async fn get_session(&self, session: SessionId) -> StoreResult<SessionSummary> {
        self.run(move |conn| {
            conn.query_row(
                "SELECT id, state, max_rounds, problem, created_at, updated_at
                 FROM sessions WHERE id = ?1",
                params![session.to_string()],
                row_to_summary,
            )
            .optional()?
            .ok_or_else(|| StoreError::NotFound(format!("session {session}")))
        })
        .await
    }

    /// List all sessions, newest first.
    pub async fn list_sessions(&self) -> StoreResult<Vec<SessionSummary>> {
        self.run(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, state, max_rounds, problem, created_at, updated_at
                 FROM sessions ORDER BY created_at DESC",
            )?;
            let rows = stmt.query_map([], row_to_summary)?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
        .await
    }

    /// Concatenate the accepted attempt's chunks for `(round, seat)`, if any.
    pub async fn accepted_text(&self, round: RoundId, seat: SeatId) -> StoreResult<Option<String>> {
        self.run(move |conn| {
            let attempt: Option<String> = conn
                .query_row(
                    "SELECT id FROM attempts
                     WHERE round_id = ?1 AND seat_id = ?2 AND status = 'accepted' LIMIT 1",
                    params![round.to_string(), seat.to_string()],
                    |r| r.get(0),
                )
                .optional()?;
            let Some(attempt) = attempt else { return Ok(None) };
            let mut stmt = conn
                .prepare("SELECT content FROM chunks WHERE attempt_id = ?1 ORDER BY seq")?;
            let text: String = stmt
                .query_map(params![attempt], |r| r.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?
                .concat();
            Ok(Some(text))
        })
        .await
    }

    /// The status of an attempt (`provisional`/`accepted`/`discarded`), if it exists.
    pub async fn attempt_status(&self, attempt: AttemptId) -> StoreResult<Option<String>> {
        self.run(move |conn| {
            Ok(conn
                .query_row(
                    "SELECT status FROM attempts WHERE id = ?1",
                    params![attempt.to_string()],
                    |r| r.get(0),
                )
                .optional()?)
        })
        .await
    }

    /// The finalized rounds of a session, in order (for export / re-open).
    pub async fn rounds(&self, session: SessionId) -> StoreResult<Vec<RoundRow>> {
        self.run(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, index_no, kind, status, focus FROM rounds
                 WHERE session_id = ?1 ORDER BY index_no, created_at",
            )?;
            let rows = stmt.query_map(params![session.to_string()], |r| {
                let id: String = r.get(0)?;
                Ok(RoundRow {
                    id: RoundId(id.parse().unwrap_or_default()),
                    index_no: r.get(1)?,
                    kind: r.get(2)?,
                    status: r.get(3)?,
                    focus: r.get(4)?,
                })
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
        .await
    }

    /// The recorded stances for a round.
    pub async fn stances(&self, round: RoundId) -> StoreResult<Vec<StanceRow>> {
        self.run(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT seat_id, stance, confidence, agree_with_json, open_questions_json
                 FROM stances WHERE round_id = ?1",
            )?;
            let rows = stmt.query_map(params![round.to_string()], |r| {
                let seat: String = r.get(0)?;
                let agree: String = r.get(3)?;
                let oq: String = r.get(4)?;
                Ok(StanceRow {
                    seat: SeatId(seat.parse().unwrap_or_default()),
                    stance: r.get(1)?,
                    confidence: r.get(2)?,
                    agree_with: serde_json::from_str(&agree).unwrap_or_default(),
                    open_questions: serde_json::from_str(&oq).unwrap_or_default(),
                })
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
        .await
    }

    /// The ruling for a round, if recorded.
    pub async fn ruling(&self, round: RoundId) -> StoreResult<Option<RulingRow>> {
        self.run(move |conn| {
            Ok(conn
                .query_row(
                    "SELECT ruling, request_user_input, next_focus, questions_json,
                            assumptions_json, summary
                     FROM rulings WHERE round_id = ?1",
                    params![round.to_string()],
                    |r| {
                        let q: String = r.get(3)?;
                        let a: String = r.get(4)?;
                        Ok(RulingRow {
                            ruling: r.get(0)?,
                            request_user_input: r.get::<_, i64>(1)? != 0,
                            next_focus: r.get(2)?,
                            questions: serde_json::from_str(&q).unwrap_or_default(),
                            assumptions: serde_json::from_str(&a).unwrap_or_default(),
                            summary: r.get(5)?,
                        })
                    },
                )
                .optional()?)
        })
        .await
    }

    /// The audit-snapshot roster for a session.
    pub async fn seats(&self, session: SessionId) -> StoreResult<Vec<SeatRow>> {
        self.run(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT seat_id, display_name, provider, base_url, model, role
                 FROM seats WHERE session_id = ?1",
            )?;
            let rows = stmt.query_map(params![session.to_string()], |r| {
                let seat: String = r.get(0)?;
                Ok(SeatRow {
                    seat: SeatId(seat.parse().unwrap_or_default()),
                    display_name: r.get(1)?,
                    provider: r.get(2)?,
                    base_url: r.get(3)?,
                    model: r.get(4)?,
                    role: r.get(5)?,
                })
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
        .await
    }

    /// The resolved user Q&A for a session, in order.
    pub async fn user_qa(&self, session: SessionId) -> StoreResult<Vec<QaRow>> {
        self.run(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT round_index, question, answer FROM user_qa
                 WHERE session_id = ?1 ORDER BY id",
            )?;
            let rows = stmt.query_map(params![session.to_string()], |r| {
                Ok(QaRow { round_index: r.get(0)?, question: r.get(1)?, answer: r.get(2)? })
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
        .await
    }
}

/// A round row (read model).
#[derive(Debug, Clone)]
pub struct RoundRow {
    pub id: RoundId,
    pub index_no: u32,
    pub kind: String,
    pub status: String,
    pub focus: Option<String>,
}

/// A recorded stance (read model).
#[derive(Debug, Clone)]
pub struct StanceRow {
    pub seat: SeatId,
    pub stance: String,
    pub confidence: f64,
    pub agree_with: Vec<SeatId>,
    pub open_questions: Vec<String>,
}

/// A recorded ruling (read model).
#[derive(Debug, Clone)]
pub struct RulingRow {
    pub ruling: String,
    pub request_user_input: bool,
    pub next_focus: String,
    pub questions: Vec<String>,
    pub assumptions: Vec<String>,
    pub summary: String,
}

/// An audit-snapshot seat (read model).
#[derive(Debug, Clone)]
pub struct SeatRow {
    pub seat: SeatId,
    pub display_name: String,
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub role: String,
}

/// A resolved user Q&A (read model).
#[derive(Debug, Clone)]
pub struct QaRow {
    pub round_index: u32,
    pub question: String,
    pub answer: String,
}

fn row_to_summary(r: &rusqlite::Row<'_>) -> rusqlite::Result<SessionSummary> {
    let id_str: String = r.get(0)?;
    let state_str: String = r.get(1)?;
    Ok(SessionSummary {
        id: SessionId(id_str.parse().unwrap_or_default()),
        state: from_tag(&state_str).unwrap_or(SessionState::Configuring),
        max_rounds: r.get(2)?,
        problem: r.get(3)?,
        created_at: r.get(4)?,
        updated_at: r.get(5)?,
    })
}

fn insert_seat(tx: &rusqlite::Transaction<'_>, session: SessionId, seat: &SeatConfig) -> StoreResult<()> {
    tx.execute(
        "INSERT INTO seats
            (session_id, seat_id, display_name, provider, base_url, model,
             model_revision, system_prompt, sampling_json, credential_ref, role)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
        params![
            session.to_string(),
            seat.id.to_string(),
            seat.display_name,
            to_tag(&seat.provider),
            seat.base_url,
            seat.model,
            Option::<String>::None,
            seat.system_prompt,
            serde_json::to_string(&seat.sampling)?,
            seat.credential_ref,
            to_tag(&seat.role),
        ],
    )?;
    Ok(())
}

/// Restrict the DB file to owner read/write (PLAN §7). Best-effort; only on unix.
fn set_owner_only(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    let _ = path;
}
