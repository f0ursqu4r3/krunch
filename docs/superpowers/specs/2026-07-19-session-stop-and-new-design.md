# Design: clean session stop + new session

_2026-07-19 — Claude, under standing overnight-autonomy grant (no check-in questions)._

## Problem

- The only way to abort a running deliberation is the "Abort" button inside the
  awaiting-user answer form (`EventLogRail`). While seats are streaming there is
  no stop control at all.
- The only way to start a new session is the "New session" button on the verdict
  overlay. Nothing mid-run.
- Latent bug: `backToSetup()` sets `sessionId = null`, and the event guard in
  `deliberation.ts` (`if (sessionId.value && e.session !== sessionId.value) return`)
  passes **all** events when `sessionId` is null — a late `verdict`/`failed`
  event from the abandoned session would yank the user out of setup back to the
  verdict overlay.

## Approach (frontend-only; backend `abandon` command already complete)

The Rust side is already clean: `abandon` cancels the run token, the engine's
`abandon()` advances state and emits `failed { reason: "abandoned" }`, and the
store routes that to the verdict overlay ("ABANDONED", dump exportable). No
engine/command changes.

1. **Store (`src/stores/deliberation.ts`)**
   - `endedSessions: Set<string>` — sessions the user walked away from.
     `handle()` drops any event whose `session` is in the set. `backToSetup()`
     adds the current session before clearing it. Fixes the stale-event hole.
   - `abandon()` — guard on `sessionId && running`, log "abort requested",
     catch invoke errors (browser preview has no Tauri).
   - `newSession()` — fire-and-forget `abandon` if running, clear
     `running`/`awaiting`, then `backToSetup()`. Problem + seat config are
     preserved (only `start()` resets runtime state).
2. **Status bar (`CockpitStatusBar.vue`)** — ABORT button visible while
   `running`. Two-step arm: first click arms ("CONFIRM ABORT", deadlock-red),
   auto-disarms after 3 s; second click aborts. Prevents accidental kill of a
   paid run; no bare keyboard shortcut for abort by design.
3. **Command palette (`CommandPalette.vue`)** — reads the store; adds
   "Abort deliberation" (room + running) and "New session" (room or verdict).
4. **Shortcuts (`shortcuts.ts`)** — `N` = new session, verdict phase only
   (safe: run already terminal). Room-phase `N` deliberately omitted.
5. **App shell (`App.vue`)** — wires the two new `ShortcutAction`s.
6. **Preview seed (`preview-seed.ts`)** — `playScriptedStream` sets
   `store.sessionId = "preview-stream"` so New-session mid-replay exercises the
   ended-session guard exactly like a live run.

## Alternatives considered

- **Stop-in-place (freeze the room, no verdict overlay):** rejected — the
  engine already emits a terminal `failed` on cancel, the overlay is
  dismissible (X) so the frozen room stays inspectable, and a second "soft
  stop" state would fork the state machine for no capability gain.
- **Bare `x`/`n` hotkeys in room:** rejected — single-keystroke destruction of
  a paid multi-model run; palette + armed button is enough.

## Error handling

- `api.abandon` failure (or browser preview): caught, logged to the event rail,
  UI state still resets on `newSession()`.
- Double-abort / abort after finish: `abandon()` no-ops unless `running`.
- Late events from an abandoned session: dropped via `endedSessions`.

## Testing

No frontend test framework in repo; verification = `vue-tsc --noEmit`,
`cargo test` (unchanged Rust must stay green), and browser preview:
`?preview=stream` (abort mid-stream, new-session mid-stream, stale events
dropped), `?preview=verdict` (`N` key), `?preview=room` (armed button).
