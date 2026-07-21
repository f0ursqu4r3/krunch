# Design: persisted settings + surfaced conversation history

_2026-07-20 — Claude, brainstormed with kyle._

## Problem

SQLite already persists every deliberation. The `krunch-store` crate holds the
full spine (`sessions → rounds → attempts → chunks`, plus `seats`, `stances`,
`rulings`, `user_qa`, `error_records`), it is wired into the Tauri app at
`src-tauri/src/lib.rs:28`, and it already does startup crash recovery. Two gaps
remain:

1. **Past conversations are stored but never surfaced.** `list_sessions` /
   `get_session` exist as commands and in `api.ts`, but no `.vue` uses them.
   There is no history browser, no way to re-open a finished session, and no
   read-model command that rehydrates a full session (rounds + stances +
   rulings + text) for display. PLAN.md §48/§57 explicitly deferred the
   "persisted session read-model" and "session replay."
2. **Settings are not persisted at all.** The roster (personas, providers,
   models, sampling), guard thresholds, mode, rounds, and problem live only in
   the Pinia `deliberation` store and reset to a hardcoded 3-seat panel every
   launch. The only persisted preference is the effects toggle
   (`localStorage['krunch-effects']`).

## Scope (agreed)

- **Conversations: view + clone-as-new.** Browse past sessions; re-open one
  read-only (replay rounds/stances/rulings/verdict/transcript); "Start new from
  this" loads the past session's problem + roster into setup as a fresh
  session. **No** engine resume of a past session; **no** timeline scrub.
- **Settings, three flavors:**
  - **Restore last setup** — auto-save the working config and restore it on
    next launch instead of the default panel. Includes the problem text.
  - **Named panel presets** — save/load named rosters. Roster + rules only,
    **no** problem (a panel is reusable across questions).
  - **App preferences** — global defaults (default mode, rounds, quorum,
    confidence floor) plus the effects toggle, moved into the DB.

## Approach: one DB, additive tables, config-as-opaque-JSON

Reuse `krunch.sqlite` and the existing single-writer `Store`. Settings and
presets are config, not audit data — we always load/save them as a whole
object, never query into them — so they are stored as **opaque JSON `TEXT`
blobs**. Rust persists and returns strings; it never parses the config, so no
new serde types and no coupling to the frontend's persona-editing shape.

Crucially, the persisted config is the **frontend editing state**
(`SeatConfig` with the `personas` array and unresolved `system_prompt`), not
the engine-facing `SessionConfig`. Persona→prompt resolution stays where it is
today, in `buildConfig()` at `start()` time. Reloading a preset therefore
restores the exact editable roster, persona chips included.

### Alternatives considered

- **Fully normalized preset tables** (mirror the `seats` audit-snapshot
  pattern): rejected — lots of schema and per-field write churn for data we
  only ever read/write as one unit. YAGNI.
- **Separate settings store** (second `.sqlite` or `localStorage`/JSON file):
  rejected — the ask is SQLite; a second `Store` duplicates the writer-thread
  machinery; `localStorage` doesn't meet the ask. (The one exception is the
  effects toggle — see §Effects.)
- **Engine resume of a past session:** rejected in favour of clone-as-new —
  resume requires the engine to rehydrate mid-flight state, far larger and
  riskier; clone-as-new delivers the practical re-run benefit for free.

## Schema

Two new tables (additive `CREATE TABLE IF NOT EXISTS`, so existing DBs upgrade
transparently on next open):

```sql
CREATE TABLE IF NOT EXISTS app_settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL          -- JSON text, opaque to Rust
);

CREATE TABLE IF NOT EXISTS panel_presets (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    config_json TEXT NOT NULL,   -- roster + rules, NO problem
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);
```

- `last_setup` is a single row in `app_settings` (`key = 'last_setup'`), config
  JSON **including** the problem text.
- App preferences are individual `app_settings` rows
  (`effects`, `default_mode`, `default_max_rounds`, `default_quorum_fraction`,
  `default_confidence_floor`).
- `schema::SCHEMA_VERSION` → `2`. `migrate()` is hardened to `UPDATE` the
  stored version to the current constant (today it only ever inserts a version
  when none exists, so a 1→2 bump would never be recorded). All changes this
  pass are additive, so no destructive migration step is needed — the version
  bump is bookkeeping so a future non-additive migration has an honest floor.

## Rust store API (new `Store` methods)

Follow the existing closure-to-writer-thread pattern:

- `get_setting(key: String) -> Option<String>`
- `set_setting(key: String, value: String) -> ()`  — upsert
- `delete_setting(key: String) -> ()`
- `list_presets() -> Vec<PresetRow>`  where `PresetRow { id, name, config_json, updated_at }`
- `save_preset(name: String, config_json: String) -> String`  — upsert on
  `name` (`INSERT … ON CONFLICT(name) DO UPDATE SET config_json, updated_at`),
  returns the row id
- `delete_preset(id: String) -> ()`

## Session read-model (new)

- **Store:** no new reads needed — `rounds`, `stances`, `ruling`,
  `accepted_text`, `seats`, `user_qa` all already exist.
- **`src-tauri/src/export.rs` / a sibling assembler:** a function that composes
  those reads into one `SessionDetailDto` (session summary + ordered rounds,
  each with its stances, ruling, and per-seat accepted text; plus the seat
  roster and user Q&A). The Markdown exporter already walks exactly this data,
  so the assembler is a structured sibling of `export_markdown`.

## Tauri commands + `api.ts`

Thin passthroughs registered in `lib.rs`'s `invoke_handler`:

- `get_setting`, `set_setting`, `list_presets`, `save_preset`, `delete_preset`
- `get_session_detail(session_id) -> SessionDetailDto` — the read-model above.

`api.ts` gains typed wrappers for each; `types.ts` gains `PresetRow` and
`SessionDetailDto`.

## Frontend wiring

A small **`settings` Pinia store** owns preferences + presets; the existing
`deliberation` store gains hydrate/snapshot helpers.

- **Boot** (`App.vue` / store `init`): load `last_setup`; if present, hydrate
  `{seats, mode, maxRounds, guard, problem}`. If absent, apply app-preference
  defaults over the current hardcoded panel. Load app preferences.
- **Autosave last-setup:** a debounced watcher (≈500 ms) on
  `{seats, mode, maxRounds, quorumFraction, confidenceFloor, problem}` writes
  the `last_setup` row. Skipped while `phase !== 'setup'` so a running/finished
  session's transient state never overwrites the saved setup.
- **Presets UI** (in `SetupScreen` / `ConvenePanel`): "Save panel as…" (name
  prompt → `save_preset` with a roster+rules snapshot, problem stripped), a
  preset picker (`list_presets`; selecting hydrates roster+rules and leaves the
  problem field untouched), and delete.
- **History + clone-as-new:** a history view (new component, reachable from the
  setup screen and the command palette) lists `list_sessions` newest-first.
  Opening an entry calls `get_session_detail` and renders it read-only through
  the existing Room/Verdict render paths in a "review" mode (no live
  subscription, no controls). A **"Start new from this"** action loads the past
  session's problem + roster back into setup as a brand-new session.

### Effects toggle

DB (`app_settings['effects']`) becomes the source of truth, but the
`localStorage['krunch-effects']` mirror is **kept** as a synchronous pre-paint
hint so there is no flash of the wrong effects level before the async DB read
resolves. On boot: read `localStorage` synchronously and apply immediately;
then reconcile from the DB once loaded (one-time import into the DB if the DB
has no `effects` row yet). On change: write both.

## Safety

- Persisted roster JSON contains `credential_ref` (a keychain lookup string)
  **only** — never the API secret, identical to the existing `seats` audit
  table. No secret ever lands in the settings DB.
- The DB file is already owner-only (`0o600`, `set_owner_only`); the new tables
  inherit it.

## Error handling

- All new commands surface `StoreError` as `String` like the existing surface;
  the frontend wrappers are `try/catch`'d and degrade quietly in browser
  preview (no Tauri), mirroring `abandon()`.
- `save_preset` on a duplicate name upserts (updates the existing preset)
  rather than erroring — matches "save panel as…" overwrite intent; the UI
  confirms before overwriting an existing name.
- Corrupt/legacy `last_setup` JSON: hydrate is wrapped so a parse failure falls
  back to defaults instead of blocking boot.
- Deleting a preset that is currently loaded in the editor leaves the editor
  state intact (the roster is already copied into the `deliberation` store).

## Testing

- **Store** (`crates/krunch-store/tests`, `tempfile` + `rusqlite`, matching the
  existing style): settings round-trip (`set`/`get`/`delete`), preset
  upsert-on-name / unique-name / list-order / delete, and `migrate()` idempotence
  across a v1→v2 open.
- **Read-model:** assemble a small scripted session and assert
  `get_session_detail` returns rounds/stances/ruling/text in order.
- **Frontend:** no test framework in repo; verification = `vue-tsc --noEmit`,
  `cargo test` green, and browser preview — `?preview=stream` /
  `?preview=verdict` unaffected, plus manual: relaunch restores last setup,
  save/load/delete a preset, open a past session read-only, "start new from
  this".

## Out of scope (explicit, deferred)

- Engine **resume** of a past/interrupted session (chose clone-as-new).
- Timeline **replay/scrub** within a re-opened session.
- Cloud sync / multi-device.
