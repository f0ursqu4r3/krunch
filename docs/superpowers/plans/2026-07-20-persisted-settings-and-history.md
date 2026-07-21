# Persisted Settings + Conversation History Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist app settings (restore-last-setup, named panel presets, app preferences) and surface the already-stored past conversations with a read-only history view and clone-as-new.

**Architecture:** Reuse `krunch.sqlite` and the existing single-writer `Store` (`crates/krunch-store`). Add three purely-additive tables — `app_settings` (KV), `panel_presets`, `session_setup` — storing config as opaque JSON `TEXT` blobs that Rust never parses. The persisted config is the **frontend editing state** (personas + unresolved addendum), captured per-session as `setup_json` so clone-as-new is faithful. Past sessions are viewed by rendering the existing `export_session` Markdown; clone-as-new rehydrates a session's `setup_json` into the setup editor as a brand-new session.

**Tech Stack:** Rust (`rusqlite` bundled, `tokio`, `serde`, `uuid`), Tauri v2 commands, Vue 3 + Pinia + `@vueuse/core`, TypeScript.

## Deviations from the spec (deliberate, flagged for reviewer)

1. **No `get_session_detail` structured read-model.** Read-only review renders the existing, HTML-escaped `export_session` Markdown through `StreamMarkdown.vue`. This reuses the audited exporter and needs zero new render code. The spec's structured-DTO/Room-replay path is dropped.
2. **New `session_setup` table + `setup_json` param on `start_deliberation`.** The spec assumed the stored config was enough for clone-as-new, but the roster's `system_prompt` is a *resolved* persona+addendum blob by the time it reaches Rust — rehydrating it would double-resolve personas. Capturing the **pre-resolution** editing snapshot per session fixes this. Legacy sessions (no `setup_json`) show clone-as-new disabled.

## Global Constraints

- **Never persist secrets.** Roster JSON stores `credential_ref` (a keychain lookup string) only — never an API key. Same as the existing `seats` audit table.
- **Config blobs are opaque to Rust.** Store/command layers pass config as `String`; only the frontend parses it. No new serde config types in Rust.
- **Additive migrations only this pass.** All new tables use `CREATE TABLE IF NOT EXISTS`; existing DBs upgrade transparently on next open. `schema::SCHEMA_VERSION` bumps `1 → 2`.
- **Browser-preview safe.** Every frontend call that hits Tauri `invoke` is guarded by `isTauri()` and/or `try/catch`, degrading to a no-op (matches `store.abandon()`).
- **Single-writer pattern.** New `Store` methods ship a closure to the writer thread via `self.run(...)`, exactly like existing methods.
- **Every git commit message ends with the trailer** `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>` (omitted from the subject-only commands below for brevity).
- **No frontend test framework exists.** Frontend "tests" are `npx vue-tsc --noEmit` (must be clean), `?preview=…` render checks where applicable, and documented manual steps in `npm run tauri dev` (persistence needs the real Tauri shell — browser preview has no `invoke`).

## File Structure

**Rust — create/modify:**
- Modify `crates/krunch-store/Cargo.toml` — add `uuid` workspace dep.
- Modify `crates/krunch-store/src/schema.rs` — three new tables, `SCHEMA_VERSION = 2`, version-bump in `migrate`.
- Modify `crates/krunch-store/src/lib.rs` — `PresetRow` struct + settings/preset/session-setup methods.
- Modify `crates/krunch-store/tests/store.rs` — round-trip + upsert + migration tests.
- Modify `src-tauri/src/commands.rs` — new commands + `setup_json` on `start_deliberation`.
- Modify `src-tauri/src/lib.rs` — register new commands in `invoke_handler`.

**Frontend — create/modify:**
- Modify `src/lib/types.ts` — `PresetRow`, `SetupSnapshot`.
- Modify `src/lib/api.ts` — wrappers + `startDeliberation` gains `setupJson`.
- Modify `src/stores/deliberation.ts` — `defaultSeats()`, `snapshotSetup()`, `hydrateSetup()`, `start()` passes `setup_json`.
- Create `src/stores/settings.ts` — preferences + presets Pinia store.
- Modify `src/App.vue` — boot restore, debounced autosave, effects DB reconcile, history dialog + `history` action.
- Modify `src/lib/shortcuts.ts` — `"history"` action + `H` key (setup).
- Modify `src/components/CommandPalette.vue` — "Browse history" item.
- Create `src/components/PresetControls.vue` — save/load/delete presets.
- Modify `src/screens/SetupScreen.vue` — mount `PresetControls` + a "History" button.
- Create `src/components/HistoryDialog.vue` — session list + read-only Markdown review + clone-as-new.

---

## Task 1: Store schema v2 (tables + migration)

**Files:**
- Modify: `crates/krunch-store/Cargo.toml`
- Modify: `crates/krunch-store/src/schema.rs`
- Test: `crates/krunch-store/tests/store.rs`

**Interfaces:**
- Produces: three tables (`app_settings`, `panel_presets`, `session_setup`); `schema::SCHEMA_VERSION == 2`.

- [ ] **Step 1: Add the `uuid` dependency** (used by `save_preset` in Task 3).

In `crates/krunch-store/Cargo.toml`, under `[dependencies]`, after the `rusqlite` line add:

```toml
uuid = { workspace = true }
```

- [ ] **Step 2: Write the failing tests**

Append to `crates/krunch-store/tests/store.rs` (these depend only on what this task ships, so the test crate keeps compiling for Tasks 2–4):

```rust
#[test]
fn schema_version_is_two() {
    assert_eq!(krunch_store::schema::SCHEMA_VERSION, 2);
}

#[tokio::test]
async fn migrate_opens_and_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("krunch.sqlite");
    let _first = Store::open(&path).unwrap();  // creates + migrates (fresh DB → v2)
    let _second = Store::open(&path).unwrap(); // re-open runs migrate() again, no error
}
```

- [ ] **Step 3: Run them to verify the version test fails**

Run: `cargo test -p krunch-store schema_version_is_two`
Expected: FAIL — `SCHEMA_VERSION` is still `1`. (`migrate_opens_and_is_idempotent` already passes; the version test is the red.)

- [ ] **Step 4: Add the DDL**

In `crates/krunch-store/src/schema.rs`, bump the version constant:

```rust
pub const SCHEMA_VERSION: i64 = 2;
```

Inside the `DDL` string literal, just before the final `CREATE TABLE IF NOT EXISTS schema_version …` line, insert:

```sql
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
```

- [ ] **Step 5: Bump the stored version in `migrate`**

Replace the body of `migrate` with:

```rust
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
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p krunch-store schema_version_is_two migrate_opens_and_is_idempotent`
Expected: both PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/krunch-store/Cargo.toml crates/krunch-store/src/schema.rs crates/krunch-store/tests/store.rs
git commit -m "feat(store): schema v2 tables for settings, presets, session setup"
```

---

## Task 2: Settings KV store methods

**Files:**
- Modify: `crates/krunch-store/src/lib.rs`
- Test: `crates/krunch-store/tests/store.rs`

**Interfaces:**
- Produces:
  - `Store::get_setting(&self, key: String) -> StoreResult<Option<String>>`
  - `Store::set_setting(&self, key: String, value: String) -> StoreResult<()>`
  - `Store::delete_setting(&self, key: String) -> StoreResult<()>`

- [ ] **Step 1: Write the failing test**

Append to `crates/krunch-store/tests/store.rs`:

```rust
#[tokio::test]
async fn settings_roundtrip_and_upsert() {
    let (_d, store) = temp_store();
    assert_eq!(store.get_setting("effects".into()).await.unwrap(), None);

    store.set_setting("effects".into(), "\"max\"".into()).await.unwrap();
    assert_eq!(store.get_setting("effects".into()).await.unwrap().as_deref(), Some("\"max\""));

    // Upsert overwrites in place.
    store.set_setting("effects".into(), "\"off\"".into()).await.unwrap();
    assert_eq!(store.get_setting("effects".into()).await.unwrap().as_deref(), Some("\"off\""));

    store.delete_setting("effects".into()).await.unwrap();
    assert_eq!(store.get_setting("effects".into()).await.unwrap(), None);
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p krunch-store settings_roundtrip_and_upsert`
Expected: FAIL — `get_setting`/`set_setting`/`delete_setting` not found.

- [ ] **Step 3: Implement the methods**

In `crates/krunch-store/src/lib.rs`, inside `impl Store`, in the `// --- reads ---` region (or just above it), add:

```rust
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
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p krunch-store settings_roundtrip_and_upsert`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/krunch-store/src/lib.rs crates/krunch-store/tests/store.rs
git commit -m "feat(store): app_settings get/set/delete"
```

---

## Task 3: Panel preset store methods

**Files:**
- Modify: `crates/krunch-store/src/lib.rs`
- Test: `crates/krunch-store/tests/store.rs`

**Interfaces:**
- Produces:
  - `pub struct PresetRow { pub id: String, pub name: String, pub config_json: String, pub updated_at: i64 }` (derives `Debug, Clone, PartialEq, serde::Serialize`)
  - `Store::list_presets(&self) -> StoreResult<Vec<PresetRow>>` (newest `updated_at` first)
  - `Store::save_preset(&self, name: String, config_json: String) -> StoreResult<String>` (upsert on `name`, returns row id)
  - `Store::delete_preset(&self, id: String) -> StoreResult<()>`

- [ ] **Step 1: Write the failing test**

Append to `crates/krunch-store/tests/store.rs`:

```rust
#[tokio::test]
async fn preset_upsert_is_keyed_by_name() {
    let (_d, store) = temp_store();
    let id1 = store.save_preset("Design jury".into(), "{\"seats\":[]}".into()).await.unwrap();
    // Same name updates in place, keeps the id.
    let id2 = store.save_preset("Design jury".into(), "{\"seats\":[1]}".into()).await.unwrap();
    assert_eq!(id1, id2);

    let all = store.list_presets().await.unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].name, "Design jury");
    assert_eq!(all[0].config_json, "{\"seats\":[1]}");

    store.delete_preset(id1).await.unwrap();
    assert!(store.list_presets().await.unwrap().is_empty());
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p krunch-store preset_upsert_is_keyed_by_name`
Expected: FAIL — `save_preset`/`list_presets`/`delete_preset` not found.

- [ ] **Step 3: Add the `PresetRow` struct**

In `crates/krunch-store/src/lib.rs`, near the other read-model structs (e.g. just after `SessionSummary`), add:

```rust
/// A saved panel preset (read model + wire type).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct PresetRow {
    pub id: String,
    pub name: String,
    pub config_json: String,
    pub updated_at: i64,
}
```

- [ ] **Step 4: Implement the methods**

Inside `impl Store`, add:

```rust
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
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p krunch-store preset_upsert_is_keyed_by_name`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/krunch-store/src/lib.rs crates/krunch-store/tests/store.rs
git commit -m "feat(store): panel preset save/list/delete (upsert by name)"
```

---

## Task 4: Session setup blob store methods

**Files:**
- Modify: `crates/krunch-store/src/lib.rs`
- Test: `crates/krunch-store/tests/store.rs`

**Interfaces:**
- Consumes: `Store::create_session` (from existing code) to get a `SessionId`.
- Produces:
  - `Store::set_session_setup(&self, session: SessionId, setup_json: String) -> StoreResult<()>`
  - `Store::get_session_setup(&self, session: SessionId) -> StoreResult<Option<String>>`

- [ ] **Step 1: Write the failing test**

Append to `crates/krunch-store/tests/store.rs`:

```rust
#[tokio::test]
async fn session_setup_blob_roundtrips() {
    let (_d, store) = temp_store();
    let s = store.create_session("k".into(), config()).await.unwrap().session_id;
    assert_eq!(store.get_session_setup(s).await.unwrap(), None);

    store.set_session_setup(s, "{\"problem\":\"x\"}".into()).await.unwrap();
    assert_eq!(
        store.get_session_setup(s).await.unwrap().as_deref(),
        Some("{\"problem\":\"x\"}")
    );
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p krunch-store session_setup_blob_roundtrips`
Expected: FAIL — `set_session_setup`/`get_session_setup` not found.

- [ ] **Step 3: Implement the methods**

Inside `impl Store`, add:

```rust
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
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p krunch-store session_setup_blob_roundtrips`
Expected: PASS.

- [ ] **Step 5: Run the whole store suite (regression)**

Run: `cargo test -p krunch-store`
Expected: all PASS, including the Task 1 migration tests and the existing lifecycle/fence/recovery tests.

- [ ] **Step 6: Commit**

```bash
git add crates/krunch-store/src/lib.rs crates/krunch-store/tests/store.rs
git commit -m "feat(store): per-session setup blob get/set"
```

---

## Task 5: Tauri commands for settings + presets

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `Store::{get_setting,set_setting,list_presets,save_preset,delete_preset}`, `krunch_store::PresetRow`.
- Produces: Tauri commands `get_setting`, `set_setting`, `list_presets`, `save_preset`, `delete_preset`.

- [ ] **Step 1: Add the imports and commands**

In `src-tauri/src/commands.rs`, extend the store import to include `PresetRow`:

```rust
use krunch_store::{PresetRow, SessionSummary, Store};
```

At the end of the file (before `core_version` or after it), add:

```rust
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
```

- [ ] **Step 2: Register the commands**

In `src-tauri/src/lib.rs`, inside `tauri::generate_handler![…]`, add these lines after `commands::save_session_dump,`:

```rust
            commands::get_setting,
            commands::set_setting,
            commands::list_presets,
            commands::save_preset,
            commands::delete_preset,
```

- [ ] **Step 3: Build the app crate**

Run: `cargo build -p krunch`
Expected: builds with no errors.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat(tauri): commands for app_settings + panel presets"
```

---

## Task 6: Capture `setup_json` on start + expose `get_session_setup`

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `Store::{set_session_setup,get_session_setup}`.
- Produces:
  - `start_deliberation` gains a `setup_json: String` parameter; on a newly-created session it persists the snapshot.
  - Tauri command `get_session_setup(session_id: String) -> Option<String>`.

- [ ] **Step 1: Add `setup_json` to `start_deliberation`**

In `src-tauri/src/commands.rs`, change the `start_deliberation` signature to add the parameter (place it after `config`):

```rust
pub async fn start_deliberation(
    app: AppHandle,
    state: State<'_, AppState>,
    idempotency_key: String,
    config: SessionConfig,
    setup_json: String,
) -> Result<StartDto, String> {
```

Then, inside the `if created.created {` block, as its **first** statement (before `let cancel = …`), persist the snapshot:

```rust
        store
            .set_session_setup(session, setup_json)
            .await
            .map_err(|e| e.to_string())?;
```

- [ ] **Step 2: Add the `get_session_setup` command**

At the end of `src-tauri/src/commands.rs`, add:

```rust
/// The pre-resolution editing snapshot for a session, if captured (used by
/// clone-as-new). `None` for legacy sessions created before capture existed.
#[tauri::command]
pub async fn get_session_setup(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<Option<String>, String> {
    let session = parse_session(&session_id)?;
    state.store.clone().get_session_setup(session).await.map_err(|e| e.to_string())
}
```

- [ ] **Step 3: Register the command**

In `src-tauri/src/lib.rs`, inside `tauri::generate_handler![…]`, add after the Task 5 lines:

```rust
            commands::get_session_setup,
```

- [ ] **Step 4: Build the app crate**

Run: `cargo build -p krunch`
Expected: builds with no errors. (The frontend `startDeliberation` caller is updated in Task 7 — the Rust side compiles independently.)

- [ ] **Step 5: Run the full Rust suite**

Run: `cargo test`
Expected: all PASS (store suite green; app crate builds).

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat(tauri): capture per-session setup_json + get_session_setup"
```

---

## Task 7: Frontend types + API wrappers

**Files:**
- Modify: `src/lib/types.ts`
- Modify: `src/lib/api.ts`

**Interfaces:**
- Produces:
  - `types.ts`: `PresetRow`, `SetupSnapshot`.
  - `api.ts`: `getSetting`, `setSetting`, `listPresets`, `savePreset`, `deletePreset`, `getSessionSetup`; `startDeliberation(idempotencyKey, config, setupJson)`.
- Consumes: existing `SeatConfig`, `InteractionMode`, `SessionConfig`, `StartDto` types.

- [ ] **Step 1: Add the types**

In `src/lib/types.ts`, after the `SessionDto` interface, add:

```ts
export interface PresetRow {
  id: string;
  name: string;
  config_json: string;
  updated_at: number;
}

/**
 * The frontend editing snapshot persisted as opaque JSON (last-setup, presets,
 * and per-session setup). camelCase because only the frontend reads it.
 */
export interface SetupSnapshot {
  problem: string;
  mode: InteractionMode;
  maxRounds: number;
  quorumFraction: number;
  confidenceFloor: number;
  seats: SeatConfig[];
}
```

- [ ] **Step 2: Update the API wrappers**

In `src/lib/api.ts`, add `PresetRow` to the type import:

```ts
import type { EngineEvent, PresetRow, SessionConfig, SessionDto, StartDto } from "./types";
```

Change the `startDeliberation` wrapper to pass `setupJson`:

```ts
  startDeliberation: (idempotencyKey: string, config: SessionConfig, setupJson: string) =>
    invoke<StartDto>("start_deliberation", { idempotencyKey, config, setupJson }),
```

Inside the `api` object (e.g. after `saveSessionDump`), add:

```ts
  getSetting: (key: string) => invoke<string | null>("get_setting", { key }),

  setSetting: (key: string, value: string) => invoke<void>("set_setting", { key, value }),

  listPresets: () => invoke<PresetRow[]>("list_presets"),

  savePreset: (name: string, configJson: string) =>
    invoke<string>("save_preset", { name, configJson }),

  deletePreset: (id: string) => invoke<void>("delete_preset", { id }),

  getSessionSetup: (sessionId: string) =>
    invoke<string | null>("get_session_setup", { sessionId }),
```

- [ ] **Step 3: Typecheck** (expected to FAIL on the `start()` caller until Task 8)

Run: `npx vue-tsc --noEmit`
Expected: one error in `src/stores/deliberation.ts` — `startDeliberation` now needs 3 args. This is fixed in Task 8. (If you prefer a green bar per task, do Task 8 before committing; otherwise commit now — the error is expected and localized.)

- [ ] **Step 4: Commit**

```bash
git add src/lib/types.ts src/lib/api.ts
git commit -m "feat(api): types + wrappers for settings, presets, session setup"
```

---

## Task 8: Deliberation store — snapshot/hydrate + pass setup on start

**Files:**
- Modify: `src/stores/deliberation.ts`

**Interfaces:**
- Consumes: `api.startDeliberation(idempotencyKey, config, setupJson)`, `SetupSnapshot`, `SeatConfig`.
- Produces (added to the store's returned object):
  - `snapshotSetup(includeProblem: boolean): SetupSnapshot`
  - `hydrateSetup(snap: SetupSnapshot, opts: { problem: boolean }): void`

- [ ] **Step 1: Import `SetupSnapshot`**

In `src/stores/deliberation.ts`, add `SetupSnapshot` to the type import:

```ts
import type {
  EngineEvent, InteractionMode, RulingKind, SeatConfig, SessionConfig, SessionState, SetupSnapshot,
} from "@/lib/types";
```

- [ ] **Step 2: Extract `defaultSeats()` and use it for the initial roster**

Just below the existing `newSeat(...)` function, add:

```ts
function defaultSeats(): SeatConfig[] {
  return [newSeat("mediator"), newSeat("panelist"), newSeat("panelist")];
}

/** Backfill any missing keys so an older/partial snapshot hydrates safely. */
function normalizeSeat(s: Partial<SeatConfig>): SeatConfig {
  return {
    id: s.id ?? crypto.randomUUID(),
    display_name: s.display_name ?? "Seat",
    provider: s.provider ?? "anthropic",
    base_url: s.base_url ?? "https://api.anthropic.com",
    model: s.model ?? "claude-sonnet-5",
    system_prompt: s.system_prompt ?? "",
    sampling: s.sampling ?? { temperature: 0.7 },
    personas: s.personas ?? [],
    credential_ref: s.credential_ref ?? "anthropic-default",
    role: s.role === "mediator" ? "mediator" : "panelist",
  };
}
```

Change the `seats` initializer line to use it:

```ts
  const seats = ref<SeatConfig[]>(defaultSeats());
```

- [ ] **Step 3: Add `snapshotSetup` / `hydrateSetup`**

Inside the store's setup function, near `buildConfig()`, add:

```ts
  function snapshotSetup(includeProblem: boolean): SetupSnapshot {
    return {
      problem: includeProblem ? problem.value : "",
      mode: mode.value,
      maxRounds: maxRounds.value,
      quorumFraction: quorumFraction.value,
      confidenceFloor: confidenceFloor.value,
      seats: JSON.parse(JSON.stringify(seats.value)) as SeatConfig[],
    };
  }
  function hydrateSetup(snap: SetupSnapshot, opts: { problem: boolean }) {
    if (opts.problem && typeof snap.problem === "string") problem.value = snap.problem;
    if (snap.mode) mode.value = snap.mode;
    if (typeof snap.maxRounds === "number") maxRounds.value = snap.maxRounds;
    if (typeof snap.quorumFraction === "number") quorumFraction.value = snap.quorumFraction;
    if (typeof snap.confidenceFloor === "number") confidenceFloor.value = snap.confidenceFloor;
    if (Array.isArray(snap.seats) && snap.seats.length) {
      seats.value = snap.seats.map(normalizeSeat);
    }
  }
```

- [ ] **Step 4: Pass the snapshot to `start()`**

In `start()`, change the `api.startDeliberation` call to include the snapshot (with problem):

```ts
    try {
      const setupJson = JSON.stringify(snapshotSetup(true));
      const result = await api.startDeliberation(crypto.randomUUID(), buildConfig(), setupJson);
      sessionId.value = result.session_id; running.value = true; phase.value = "room"; appendLog("state_changed", "session convened");
    }
    catch (error) { startError.value = String(error); }
```

- [ ] **Step 5: Export the new helpers**

In the store's `return { … }`, add `snapshotSetup, hydrateSetup` to the returned object (e.g. next to `buildConfig`-adjacent helpers like `addPanelist`).

```ts
    problem, mode, maxRounds, quorumFraction, confidenceFloor, seats, panelists, mediator, validation, addPanelist, removeSeat, loadDemoPanel, snapshotSetup, hydrateSetup,
```

- [ ] **Step 6: Typecheck**

Run: `npx vue-tsc --noEmit`
Expected: clean (the Task 7 error is now resolved).

- [ ] **Step 7: Commit**

```bash
git add src/stores/deliberation.ts
git commit -m "feat(store): setup snapshot/hydrate + pass setup_json on convene"
```

---

## Task 9: Settings Pinia store

**Files:**
- Create: `src/stores/settings.ts`

**Interfaces:**
- Consumes: `api.{listPresets,savePreset,deletePreset,getSetting,setSetting}`, `isTauri()`, `SetupSnapshot`, `PresetRow`.
- Produces: `useSettings()` store exposing `presets`, `loadPresets()`, `savePreset(name, snap)`, `removePreset(id)`, `saveLastSetup(snap)`, `loadLastSetup()`, `persistEffects(value)`, `reconcileEffects(current)`.

- [ ] **Step 1: Create the store**

Create `src/stores/settings.ts`:

```ts
import { defineStore } from "pinia";
import { ref } from "vue";
import { api, isTauri } from "@/lib/api";
import type { PresetRow, SetupSnapshot } from "@/lib/types";

const LAST_SETUP_KEY = "last_setup";
const EFFECTS_KEY = "effects";
const EFFECTS_MIRROR = "krunch-effects"; // synchronous pre-paint hint

export type EffectsLevel = "off" | "ambient" | "max";

export const useSettings = defineStore("settings", () => {
  const presets = ref<PresetRow[]>([]);

  async function loadPresets() {
    if (!isTauri()) return;
    try { presets.value = await api.listPresets(); } catch { /* preview */ }
  }
  async function savePreset(name: string, snap: SetupSnapshot) {
    if (!isTauri()) return;
    try { await api.savePreset(name, JSON.stringify(snap)); await loadPresets(); } catch { /* preview */ }
  }
  async function removePreset(id: string) {
    if (!isTauri()) return;
    try { await api.deletePreset(id); await loadPresets(); } catch { /* preview */ }
  }

  async function saveLastSetup(snap: SetupSnapshot) {
    if (!isTauri()) return;
    try { await api.setSetting(LAST_SETUP_KEY, JSON.stringify(snap)); } catch { /* preview */ }
  }
  async function loadLastSetup(): Promise<SetupSnapshot | null> {
    if (!isTauri()) return null;
    try {
      const raw = await api.getSetting(LAST_SETUP_KEY);
      return raw ? (JSON.parse(raw) as SetupSnapshot) : null;
    } catch { return null; }
  }

  /** Write effects to both the localStorage mirror (sync) and the DB (source of truth). */
  async function persistEffects(value: EffectsLevel) {
    localStorage.setItem(EFFECTS_MIRROR, value);
    if (!isTauri()) return;
    try { await api.setSetting(EFFECTS_KEY, JSON.stringify(value)); } catch { /* preview */ }
  }
  /** DB wins if present; else import the current (localStorage-seeded) value into the DB once. */
  async function reconcileEffects(current: EffectsLevel): Promise<EffectsLevel> {
    if (!isTauri()) return current;
    try {
      const raw = await api.getSetting(EFFECTS_KEY);
      if (raw) return JSON.parse(raw) as EffectsLevel;
      await api.setSetting(EFFECTS_KEY, JSON.stringify(current));
      return current;
    } catch { return current; }
  }

  return {
    presets, loadPresets, savePreset, removePreset,
    saveLastSetup, loadLastSetup, persistEffects, reconcileEffects,
  };
});
```

- [ ] **Step 2: Typecheck**

Run: `npx vue-tsc --noEmit`
Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add src/stores/settings.ts
git commit -m "feat(store): settings pinia store (presets, last-setup, effects)"
```

---

## Task 10: Boot restore + autosave + effects reconcile (App.vue)

**Files:**
- Modify: `src/App.vue`

**Interfaces:**
- Consumes: `useSettings()`, `store.{hydrateSetup,snapshotSetup,phase}`, `watchDebounced` from `@vueuse/core`.
- Produces: on launch, last-setup is restored and presets loaded; edits autosave; effects reconcile to DB. Adds a `history` ref + `"history"` action handling (dialog wired in Task 12).

- [ ] **Step 1: Add imports**

In `src/App.vue` `<script setup>`, add:

```ts
import { watchDebounced } from "@vueuse/core";
import { useSettings } from "@/stores/settings";
```

And after `const store = useDeliberation();`, add:

```ts
const settings = useSettings();
```

- [ ] **Step 2: Route effects writes through the settings store**

Replace the existing effects watcher line:

```ts
watch(effects, (value) => { localStorage.setItem("krunch-effects", value); store.setReducedEffects(value === "off" || reducedMotion.value || autoReduced.value); });
```

with:

```ts
watch(effects, (value) => { void settings.persistEffects(value); store.setReducedEffects(value === "off" || reducedMotion.value || autoReduced.value); });
```

- [ ] **Step 3: Add the debounced autosave watcher**

After the effects watchers, add:

```ts
// Persist the working setup (debounced) so the next launch restores it. Only
// while editing — never let a running/finished session's state overwrite it.
watchDebounced(
  () => [store.problem, store.mode, store.maxRounds, store.quorumFraction, store.confidenceFloor, store.seats] as const,
  () => { if (store.phase === "setup") void settings.saveLastSetup(store.snapshotSetup(true)); },
  { debounce: 500, deep: true },
);
```

- [ ] **Step 4: Restore on boot + load presets + reconcile effects**

In `onMounted`, immediately after `store.init();`, add:

```ts
    const restored = await settings.loadLastSetup();
    if (restored) store.hydrateSetup(restored, { problem: true });
    void settings.loadPresets();
    effects.value = await settings.reconcileEffects(effects.value);
```

- [ ] **Step 5: Add the `history` ref + action handling**

After `const palette = ref(false);` add:

```ts
const history = ref(false);
```

In the `act(...)` function, add a `history` branch (before the `focus-seat` branch):

```ts
if (action === "history") return void (history.value = true);
```

(The `HistoryDialog` element is added to the template in Task 12. TypeScript is satisfied now because `history` is a declared ref.)

- [ ] **Step 6: Typecheck**

Run: `npx vue-tsc --noEmit`
Expected: FAIL — `"history"` is not yet a member of `ShortcutAction`. Fixed in Task 11 Step 1. (Do Task 11 next; or temporarily this task's typecheck is red only on the action union.)

- [ ] **Step 7: Commit**

```bash
git add src/App.vue
git commit -m "feat(app): restore last setup, autosave, reconcile effects to DB"
```

---

## Task 11: `history` shortcut/action + presets UI

**Files:**
- Modify: `src/lib/shortcuts.ts`
- Modify: `src/components/CommandPalette.vue`
- Create: `src/components/PresetControls.vue`
- Modify: `src/screens/SetupScreen.vue`

**Interfaces:**
- Consumes: `useDeliberation()`, `useSettings()`, `store.{snapshotSetup,hydrateSetup}`, `SetupSnapshot`.
- Produces: `ShortcutAction` includes `"history"`; `H` opens history in setup; palette "Browse history" item; `PresetControls.vue` mounted in `SetupScreen`.

- [ ] **Step 1: Add the `history` action + `H` key**

In `src/lib/shortcuts.ts`, extend the union:

```ts
export type ShortcutAction = "palette" | "convene" | "add-seat" | "export" | "help" | "focus-seat" | "escape" | "abort" | "new-session" | "history";
```

Add a key mapping (in `shortcutFor`, near the setup-phase `a`/`c` lines):

```ts
  if (phase === "setup" && event.key.toLowerCase() === "h") return { action: "history" };
```

- [ ] **Step 2: Add the palette item**

In `src/components/CommandPalette.vue`, add to the `commands` array (after the `add-seat` entry):

```ts
  { id: "history", label: "Browse history", keys: "H", show: props.phase === "setup" },
```

- [ ] **Step 3: Create `PresetControls.vue`**

Create `src/components/PresetControls.vue`:

```vue
<script setup lang="ts">
import { onMounted, ref } from "vue";
import { Save, Trash2 } from "@lucide/vue";
import { useDeliberation } from "@/stores/deliberation";
import { useSettings } from "@/stores/settings";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { SetupSnapshot } from "@/lib/types";

const store = useDeliberation();
const settings = useSettings();
const name = ref("");

onMounted(() => settings.loadPresets());

async function save() {
  const n = name.value.trim();
  if (!n) return;
  const existing = settings.presets.find((p) => p.name === n);
  if (existing && !window.confirm(`Overwrite preset "${n}"?`)) return;
  await settings.savePreset(n, store.snapshotSetup(false)); // roster + rules, no problem
  name.value = "";
}
function load(configJson: string) {
  try {
    const snap = JSON.parse(configJson) as SetupSnapshot;
    store.hydrateSetup(snap, { problem: false }); // keep the current problem text
  } catch { /* corrupt preset — ignore */ }
}
async function remove(id: string) { await settings.removePreset(id); }
</script>

<template>
  <div class="terminal-panel p-4">
    <p class="mb-2.5 font-mono text-[11px] uppercase tracking-[0.14em] text-brass">Panel presets</p>
    <div class="flex gap-2">
      <Input v-model="name" placeholder="Name this panel…" class="h-8 bg-bg-deep text-[11px]" @keydown.enter="save" />
      <Button size="xs" variant="outline" class="border-brass/50 text-brass" :disabled="!name.trim()" @click="save"><Save data-icon="inline-start" />Save</Button>
    </div>
    <ul v-if="settings.presets.length" class="mt-3 space-y-1.5 border-t border-line pt-3 font-mono text-[11px]">
      <li v-for="p in settings.presets" :key="p.id" class="flex items-center gap-2">
        <button class="flex-1 truncate text-left text-fg-muted hover:text-brass" @click="load(p.config_json)">{{ p.name }}</button>
        <button class="text-fg-faint hover:text-deadlock" :aria-label="`Delete ${p.name}`" @click="remove(p.id)"><Trash2 class="size-3.5" /></button>
      </li>
    </ul>
    <p v-else class="mt-3 border-t border-line pt-3 font-mono text-[10px] text-fg-faint">No presets yet. Seat a panel, then save it.</p>
  </div>
</template>
```

- [ ] **Step 4: Mount `PresetControls` + a History button in `SetupScreen`**

In `src/screens/SetupScreen.vue`, add to the `<script setup>` imports:

```ts
import { History } from "@lucide/vue";
import PresetControls from "@/components/PresetControls.vue";
```

Add an emit so the header button can ask `App.vue` to open history (App owns the dialog). At the top of `<script setup>`, after the imports, add:

```ts
const emit = defineEmits<{ history: [] }>();
```

In the template header (`<header …>`), change the demo button row to include a History button — replace the existing `<Button … @click="store.loadDemoPanel()">…</Button>` with:

```vue
        <div class="flex gap-2">
          <Button size="sm" variant="outline" @click="emit('history')"><History data-icon="inline-start" />History <kbd class="ml-1 text-fg-faint">H</kbd></Button>
          <Button size="sm" variant="outline" class="border-brass/50 text-brass" @click="store.loadDemoPanel()"><Sparkles data-icon="inline-start" />Load demo panel</Button>
        </div>
```

Mount `PresetControls` in the right-hand column under `ConvenePanel` — change the hero-card wrapper:

```vue
        <div ref="heroCard" class="space-y-5"><ConvenePanel variant="card" /><PresetControls /></div>
```

- [ ] **Step 5: Wire `SetupScreen`'s history emit in `App.vue`**

In `src/App.vue` template, update the `SetupScreen` element to forward the event to the same action path:

```vue
<SetupScreen v-if="store.phase === 'setup'" class="boot" @history="history = true" />
```

- [ ] **Step 6: Typecheck**

Run: `npx vue-tsc --noEmit`
Expected: clean (Task 10's `"history"` action error is now resolved).

- [ ] **Step 7: Preview render check**

Start the dev server (preview) and confirm the setup screen renders the presets panel and History button without console errors.

Run: preview `?preview=` (setup is the default phase). Verify via `read_console_messages` that there are no errors and the "Panel presets" card + "History" button appear.

- [ ] **Step 8: Commit**

```bash
git add src/lib/shortcuts.ts src/components/CommandPalette.vue src/components/PresetControls.vue src/screens/SetupScreen.vue src/App.vue
git commit -m "feat(setup): panel presets UI + history entry point"
```

---

## Task 12: History dialog — list + read-only review

**Files:**
- Create: `src/components/HistoryDialog.vue`
- Modify: `src/App.vue`

**Interfaces:**
- Consumes: `api.{listSessions,exportSession}`, `isTauri()`, `SessionDto`, `StreamMarkdown` (`:text` prop), the `dialog` UI components.
- Produces: `<HistoryDialog v-model:open="history" />` rendered in `App.vue`; a `v-model:open` boolean + an internal session list and Markdown detail pane. (Clone-as-new button is added in Task 13.)

- [ ] **Step 1: Create `HistoryDialog.vue`**

Create `src/components/HistoryDialog.vue`:

```vue
<script setup lang="ts">
import { ref, watch } from "vue";
import { api, isTauri } from "@/lib/api";
import type { SessionDto } from "@/lib/types";
import StreamMarkdown from "@/components/StreamMarkdown.vue";
import { Dialog, DialogScrollContent, DialogHeader, DialogTitle, DialogDescription } from "@/components/ui/dialog";

const open = defineModel<boolean>("open", { required: true });

const sessions = ref<SessionDto[]>([]);
const selected = ref<SessionDto | null>(null);
const detail = ref<string>("");
const loading = ref(false);

async function refresh() {
  if (!isTauri()) { sessions.value = []; return; }
  try { sessions.value = await api.listSessions(); } catch { sessions.value = []; }
}
async function openSession(s: SessionDto) {
  selected.value = s;
  loading.value = true;
  detail.value = "";
  try { detail.value = await api.exportSession(s.id); }
  catch (e) { detail.value = `_Could not load this session: ${String(e)}_`; }
  finally { loading.value = false; }
}

// Refresh the list each time the dialog opens; reset the detail pane.
watch(open, (isOpen) => { if (isOpen) { selected.value = null; detail.value = ""; void refresh(); } });

function fmt(ts: number): string { return new Date(ts).toLocaleString(); }
</script>

<template>
  <Dialog v-model:open="open">
    <DialogScrollContent class="max-w-4xl">
      <DialogHeader>
        <DialogTitle class="font-display text-brass">Past deliberations</DialogTitle>
        <DialogDescription class="font-mono text-[10px] uppercase tracking-[0.14em]">Read-only review of stored sessions.</DialogDescription>
      </DialogHeader>
      <div class="grid min-h-0 gap-4 md:grid-cols-[18rem_minmax(0,1fr)]">
        <ul class="max-h-[60vh] space-y-1 overflow-y-auto border-r border-line pr-3 font-mono text-[11px]">
          <li v-if="!sessions.length" class="text-fg-faint">No stored sessions.</li>
          <li v-for="s in sessions" :key="s.id">
            <button
              class="w-full rounded px-2 py-1.5 text-left hover:bg-surface"
              :class="selected?.id === s.id ? 'bg-surface text-brass' : 'text-fg-muted'"
              @click="openSession(s)"
            >
              <span class="line-clamp-2">{{ s.problem || "(untitled)" }}</span>
              <span class="mt-0.5 block text-[10px] text-fg-faint">{{ s.state }} · {{ fmt(s.created_at) }}</span>
            </button>
          </li>
        </ul>
        <div class="max-h-[60vh] overflow-y-auto">
          <p v-if="!selected" class="font-mono text-[11px] text-fg-faint">Select a session to review it.</p>
          <p v-else-if="loading" class="font-mono text-[11px] text-fg-faint">Loading…</p>
          <StreamMarkdown v-else :text="detail" />
        </div>
      </div>
    </DialogScrollContent>
  </Dialog>
</template>
```

- [ ] **Step 2: Render the dialog in `App.vue`**

In `src/App.vue`, add the import:

```ts
import HistoryDialog from "@/components/HistoryDialog.vue";
```

In the template, after the `CommandPalette` element, add:

```vue
    <HistoryDialog v-model:open="history" />
```

- [ ] **Step 3: Typecheck**

Run: `npx vue-tsc --noEmit`
Expected: clean.

- [ ] **Step 4: Manual verification (real app — persistence needs Tauri)**

Run: `npm run tauri dev`
Steps: run at least one deliberation to completion, open History (button or `H`), confirm the session appears in the list and clicking it renders the exported Markdown transcript. Browser preview will show an empty list (no `invoke`) — that is expected.

- [ ] **Step 5: Commit**

```bash
git add src/components/HistoryDialog.vue src/App.vue
git commit -m "feat(history): browse + read-only markdown review of past sessions"
```

---

## Task 13: Clone-as-new from a past session

**Files:**
- Modify: `src/components/HistoryDialog.vue`

**Interfaces:**
- Consumes: `api.getSessionSetup`, `useDeliberation().hydrateSetup`, `SetupSnapshot`.
- Produces: a "Start new from this" button in the review pane; enabled only when the session has a captured `setup_json` (new sessions), disabled for legacy sessions.

- [ ] **Step 1: Add clone state + logic**

In `src/components/HistoryDialog.vue` `<script setup>`, add the store import and clone handling:

```ts
import { useDeliberation } from "@/stores/deliberation";
import type { SetupSnapshot } from "@/lib/types";

const store = useDeliberation();
const setupRaw = ref<string | null>(null);
```

In `openSession`, after setting `selected.value = s;`, fetch the setup blob (so the button knows if clone is possible):

```ts
  setupRaw.value = null;
  try { setupRaw.value = isTauri() ? await api.getSessionSetup(s.id) : null; } catch { setupRaw.value = null; }
```

Add the clone function:

```ts
function cloneAsNew() {
  if (!setupRaw.value) return;
  try {
    const snap = JSON.parse(setupRaw.value) as SetupSnapshot;
    store.hydrateSetup(snap, { problem: true }); // load problem + roster into the setup editor
    open.value = false; // dialog only opens from the setup phase, so we land back on setup
  } catch { /* corrupt snapshot — leave the editor untouched */ }
}
```

- [ ] **Step 2: Add the button to the review pane**

In the template, inside the detail `<div>` (the `md:grid` right column), add a button above the `StreamMarkdown`, shown once a session is selected:

```vue
          <div v-if="selected && !loading" class="mb-3 flex items-center gap-3">
            <Button size="sm" variant="outline" class="border-consensus/45 text-consensus" :disabled="!setupRaw" @click="cloneAsNew">Start new from this</Button>
            <span v-if="!setupRaw" class="font-mono text-[10px] text-fg-faint">setup not captured for this session</span>
          </div>
```

Add the `Button` import at the top of `<script setup>`:

```ts
import { Button } from "@/components/ui/button";
```

- [ ] **Step 3: Typecheck**

Run: `npx vue-tsc --noEmit`
Expected: clean.

- [ ] **Step 4: Manual verification (real app)**

Run: `npm run tauri dev`
Steps: run a new deliberation (this captures `setup_json`), open History, select it, confirm "Start new from this" is enabled; click it and confirm the setup editor is populated with that session's problem + roster (personas intact) and the dialog closes. For a session created before this feature, the button is disabled with the "setup not captured" note.

- [ ] **Step 5: Commit**

```bash
git add src/components/HistoryDialog.vue
git commit -m "feat(history): clone-as-new from a captured session setup"
```

---

## Final verification

- [ ] **Rust:** `cargo test` — all green.
- [ ] **Types:** `npx vue-tsc --noEmit` — clean.
- [ ] **Full build:** `npm run build` — succeeds (`vue-tsc --noEmit && vite build`).
- [ ] **Manual (Tauri):** `npm run tauri dev` — relaunch restores last setup (problem + roster); save/load/delete a preset (problem preserved); effects level persists across relaunch; History lists past sessions, review renders, clone-as-new repopulates setup; browser `?preview=stream` / `?preview=verdict` unaffected.

## Self-review notes (spec coverage)

- Restore-last-setup → Tasks 4/8/9/10 (`session_setup` unused here; last-setup via `app_settings['last_setup']` + boot restore + autosave). ✓
- Named presets → Tasks 3/9/11. ✓
- App preferences / effects to DB with localStorage mirror → Tasks 2/9/10. ✓
- View past conversations (read-only) → Task 12 (renders `export_session`). ✓
- Clone-as-new → Tasks 4/6/13 (`session_setup` + `get_session_setup`). ✓
- Credentials never persisted as secrets → roster JSON carries `credential_ref` only (Global Constraints). ✓
- Resume + replay/scrub → out of scope (unchanged). ✓
