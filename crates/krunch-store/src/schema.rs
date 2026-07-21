//! SQLite DDL + migration (PLAN §7). One relational spine:
//! `sessions → rounds → attempts → chunks`, with `seats` (audit snapshot),
//! `stances`, `rulings`, `user_qa`, and `error_records`.

use rusqlite::Connection;

/// Bump when the DDL changes; `migrate` is idempotent per version.
pub const SCHEMA_VERSION: i64 = 2;

const DDL: &str = r#"
CREATE TABLE IF NOT EXISTS sessions (
    id               TEXT PRIMARY KEY,
    idempotency_key  TEXT UNIQUE,
    state            TEXT NOT NULL,
    mode             TEXT NOT NULL,
    max_rounds       INTEGER NOT NULL,
    quorum_fraction  REAL NOT NULL,
    confidence_floor REAL NOT NULL,
    problem          TEXT NOT NULL,
    created_at       INTEGER NOT NULL,
    updated_at       INTEGER NOT NULL
);

-- Roster captured at start: an audit snapshot, never the credential itself.
CREATE TABLE IF NOT EXISTS seats (
    session_id     TEXT NOT NULL REFERENCES sessions(id),
    seat_id        TEXT NOT NULL,
    display_name   TEXT NOT NULL,
    provider       TEXT NOT NULL,
    base_url       TEXT NOT NULL,
    model          TEXT NOT NULL,
    model_revision TEXT,
    system_prompt  TEXT NOT NULL,
    sampling_json  TEXT NOT NULL,
    credential_ref TEXT NOT NULL,
    role           TEXT NOT NULL,
    PRIMARY KEY (session_id, seat_id)
);

CREATE TABLE IF NOT EXISTS rounds (
    id         TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id),
    index_no   INTEGER NOT NULL,
    kind       TEXT NOT NULL,   -- deliberation | finalization
    status     TEXT NOT NULL,   -- running | finalized | abandoned
    focus      TEXT,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_rounds_session ON rounds(session_id, index_no);

CREATE TABLE IF NOT EXISTS attempts (
    id         TEXT PRIMARY KEY,
    round_id   TEXT NOT NULL REFERENCES rounds(id),
    seat_id    TEXT NOT NULL,
    attempt_no INTEGER NOT NULL,
    status     TEXT NOT NULL,   -- provisional | accepted | discarded
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_attempts_round_seat ON attempts(round_id, seat_id);

-- Streamed token chunks. Bounded batches flushed by the single writer.
CREATE TABLE IF NOT EXISTS chunks (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    attempt_id TEXT NOT NULL REFERENCES attempts(id),
    seq        INTEGER NOT NULL,
    content    TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_chunks_attempt ON chunks(attempt_id, seq);

CREATE TABLE IF NOT EXISTS stances (
    round_id            TEXT NOT NULL REFERENCES rounds(id),
    seat_id             TEXT NOT NULL,
    stance              TEXT NOT NULL,
    confidence          REAL NOT NULL,
    agree_with_json     TEXT NOT NULL,
    open_questions_json TEXT NOT NULL,
    PRIMARY KEY (round_id, seat_id)
);

CREATE TABLE IF NOT EXISTS rulings (
    round_id           TEXT PRIMARY KEY REFERENCES rounds(id),
    ruling             TEXT NOT NULL,
    request_user_input INTEGER NOT NULL,
    next_focus         TEXT NOT NULL,
    questions_json     TEXT NOT NULL,
    assumptions_json   TEXT NOT NULL,
    summary            TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS user_qa (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id   TEXT NOT NULL REFERENCES sessions(id),
    round_index  INTEGER NOT NULL,
    question     TEXT NOT NULL,
    answer       TEXT NOT NULL,
    created_at   INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS error_records (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id          TEXT NOT NULL,
    round_id            TEXT,
    seat_id             TEXT,
    attempt_id          TEXT,
    kind                TEXT NOT NULL,
    http_status         INTEGER,
    retry_count         INTEGER NOT NULL,
    deadline_hit        INTEGER NOT NULL,
    provider_request_id TEXT,
    detail              TEXT NOT NULL,
    created_at          INTEGER NOT NULL
);

-- App preferences + restore-last-setup singleton (opaque JSON values).
CREATE TABLE IF NOT EXISTS app_settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Named, reusable panel presets (roster + rules, NO problem). Opaque JSON.
CREATE TABLE IF NOT EXISTS panel_presets (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    config_json TEXT NOT NULL,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

-- Per-session pre-resolution editing snapshot, for faithful clone-as-new.
CREATE TABLE IF NOT EXISTS session_setup (
    session_id TEXT PRIMARY KEY REFERENCES sessions(id),
    setup_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);
"#;

/// Create tables if absent and stamp the schema version.
pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(DDL)?;
    let existing: Option<i64> = conn
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |r| r.get(0))
        .ok();
    match existing {
        None => {
            conn.execute("INSERT INTO schema_version (version) VALUES (?1)", [SCHEMA_VERSION])?;
        }
        Some(v) if v < SCHEMA_VERSION => {
            conn.execute("UPDATE schema_version SET version = ?1", [SCHEMA_VERSION])?;
        }
        _ => {}
    }
    Ok(())
}
