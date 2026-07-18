# Build Log: Selectable seat personalities

Spec: `docs/superpowers/specs/2026-07-17-seat-personalities-design.md`
Plan: `docs/superpowers/plans/2026-07-17-seat-personalities.md`

## Act 3 — Build
Builder: Codex (gpt-5.6-terra, reasoning high) — codex-cli 0.144.0, `codex exec --yolo`, one shot, 0 fix rounds. Branch `feat/seat-personalities`.

### Round 1 — Codex build
Implemented Tasks 1–4, committed per-task (`925771c`, `1b287f3`, `9ae6f82`, `8bf8231`). Files:
- `crates/krunch-core/src/config.rs` — additive `#[serde(default)] personas: Vec<String>` + two deserialization tests.
- `crates/krunch-engine/tests/deliberation.rs`, `crates/krunch-store/tests/store.rs` — added `personas: vec![]` to their explicit `SeatConfig` test literals (the plan under-specified these; the grep note caught them).
- `src/lib/personas.ts` — catalog (8 temperaments, 8 domains, 4 mediator), `NONE` sentinel, `personaById`/`personasForGroup`/`groupsForRole`, pure `resolveSystemPrompt`.
- `src/lib/types.ts` — `personas: string[]` on `SeatConfig`.
- `src/stores/deliberation.ts` — `newSeat` default `[]`, `buildConfig` resolves personas→`system_prompt`, demo panel personas.
- `src/components/SeatEditor.vue` — group-proxy pickers (temperament/domain for panelist, mediator for mediator), role-reconcile watch, relabeled CUSTOM (ADDENDUM).
Proof reported green. Deviation reported: none in implementation; browser walkthrough left to Claude (no browser surface in Codex's env).

### Claude's verdict — APPROVED
Read the full diff like a PR — exactly faithful to the plan, no scope creep:
- Rust: additive field + `#[serde(default)]`; both extra test literals correctly fixed; engine untouched.
- `resolveSystemPrompt` is the exact pure function (canonical order, unknown ids skipped, trimmed addendum, empty→"").
- `buildConfig` resolves per seat while `personas` still rides the wire (audit snapshot).
- `SeatEditor`: role-aware pickers via `groupProxy` + `NONE` sentinel; role-change watch drops invalid personas; addendum always visible.
Proof re-run by Claude: `npm run build` ✓ (type-check + bundle); `cargo test` ✓ **101 passed / 0 failed** (incl. `seat_config_personas_defaults_when_absent`, `seat_config_personas_roundtrip`).
Browser (Codex couldn't): panelist seat shows TEMPERAMENT + DOMAIN EXPERT; mediator shows only MEDIATOR PERSONA; **Load demo panel** pre-selects Foreman=Neutral Foreman, Juror A=Optimist+Designer, Juror B=Skeptic+Engineer; PREFLIGHT → "all systems ready". **No console errors.** 0 fix rounds.

Process note: per the plan's TDD "Commit" steps + the build contract, Codex committed per task. Those commits are isolated on `feat/seat-personalities`; nothing merged or pushed — the human sign-off gate (merge to `main`) is intact.
