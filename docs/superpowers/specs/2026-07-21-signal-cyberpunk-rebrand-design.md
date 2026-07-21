# krunch rebrand — "Signal" (sci-fi / cyberpunk ops-terminal)

**Date:** 2026-07-21
**Status:** Approved design — ready for implementation plan
**Type:** Full visual rebrand (no backend, data model, or store-logic changes)

## Summary

Replace krunch's current "deliberation chamber" identity (warm brass/candlelight,
Young Serif, old-world jury room) with a sci-fi/cyberpunk **ops-terminal** identity:
a live signal room, pure-black base, neon green primary, red for alerts, with a
new signature moment — an **oscilloscope** where each panelist's signal
phase-locks into one clean green trace as the panel converges, and desyncs into
red noise at deadlock.

This is a surface rebrand. Screen structure, component boundaries, the Rust
backend, the wire types, and all store logic in
[src/stores/deliberation.ts](../../../src/stores/deliberation.ts) stay unchanged.
The one genuinely new build is the oscilloscope component.

## Concept & personality

krunch becomes a **deliberation ops terminal** — a live signal room where a panel
of models transmits and you watch their signals lock into one. Three words:
**live · high-stakes · instrumented.** The verdict reads as a decoded transmission
on record.

This rewrites [.impeccable.md](../../../.impeccable.md) wholesale. New anti-references:
warm/skeuomorphic/old-world, parchment, serif gravitas, candlelight.

## Palette (green/red terminal)

Keep the **token architecture** in [src/style.css](../../../src/style.css) — same
CSS variable names so component churn stays minimal — and remap the *values*. One
mechanical rename: `brass` → `signal` (the name is semantically wrong now),
applied as a sweep across component classes (`text-brass`, `bg-brass`,
`border-brass`, `--color-brass*`, etc.) and the `@theme inline` block.

| token | new value (oklch, approximate — tune during impl) | role |
|---|---|---|
| `--bg-deep` | `oklch(0 0 0)` pure black | void behind panels |
| `--bg` | `oklch(0.15 0.012 165)` near-black, faint green | app base |
| `--surface` | `oklch(0.19 0.014 165)` | panels |
| `--surface-2` | `oklch(0.235 0.016 165)` | raised chrome |
| `--surface-3` | `oklch(0.285 0.018 165)` | hover/active |
| `--fg` | `oklch(0.94 0.02 165)` phosphor white-green | text |
| `--fg-muted` | `oklch(0.75 0.03 165)` | secondary text |
| `--fg-faint` | `oklch(0.57 0.03 165)` | labels/telemetry |
| `--signal` (was `--brass`) | `oklch(0.85 0.21 160)` neon green | primary accent |
| `--signal-bright` (was `--brass-bright`) | `oklch(0.90 0.23 160)` | hot accent |
| `--signal-deep` (was `--brass-deep`) | `oklch(0.62 0.16 160)` | pressed/deep |
| `--consensus` | `oklch(0.88 0.24 158)` bright lock-green | converged |
| `--continue` | dim `--signal` | in progress |
| `--deadlock` / `--danger` | `oklch(0.62 0.25 22)` neon red | deadlock / alert |
| `--line` | cool green-graphite hairline | borders |
| `--line-strong` | brighter green-graphite | strong borders |

shadcn/reka bindings (`--primary`, `--ring`, `--destructive`, …) re-point to the
remapped tokens; `--primary` → `--signal`, `--destructive` → `--deadlock`, etc.
Base kept slightly lifted (not literal `#000` for surfaces) so panels read against
the pure-black void and neon doesn't fatigue.

## Typography (Monaspace family only — no new font dependency)

All faces already exist in [src/assets/fonts](../../../src/assets/fonts). Choose
texture per role:

| role | face |
|---|---|
| big headings / wordmark / verdict title | **Departure Mono** (pixel) — large sizes only |
| UI / chrome / buttons | **Monaspace Neon** (neo-grotesque) |
| telemetry (seat ids, %, round counters, model names) | **Monaspace Krypton** (mechanical) |
| streamed deliberation prose | **Monaspace Argon** (humanist) — legible over minutes |
| verdict body / record | **Monaspace Xenon** (slab) — editorial weight |

`--font-display`, `--font-sans`, `--font-mono` in the `@theme` block get repointed;
add explicit utility faces where a component needs a specific texture. Young Serif
and Hanken Grotesk drop out of active use but stay in the repo.

## Effects kit

All wired to the existing `--effects-intensity` variable and the `off / ambient /
max` control, the perf auto-reduce loop in [src/App.vue](../../../src/App.vue), and
the `prefers-reduced-motion` guard. Chosen effects:

- **Phosphor glow / bloom** — neon text and borders emit a soft glow via layered
  `text-shadow` / `box-shadow`; green glows, red alerts glow hotter. Replaces the
  warm top-edge gradient on `.terminal-panel`.
- **Grid + dataflow background** — a faint moving dot/line grid behind everything
  (new fixed layer replacing `.chamber-glow`'s radial), plus slow drifting dataflow
  lines. Transform-only animation to stay GPU-cheap.
- **HUD chrome + glitch** — corner brackets, notched/clipped panel edges
  (`clip-path`), bracket labels (`[ SEAT 03 ]`), tick marks, a telemetry ticker;
  RGB-split glitch flicker on state changes / errors; a short boot/handshake
  sequence on launch (replaces the current "Krunch / the deliberation chamber"
  overlay) and a "CONVENING… / channels opening" sequence when the panel convenes.
- **Typewriter** — kept exactly as-is (already on-theme for a terminal).

**Excluded:** scanlines / CRT curvature — deliberately skipped to keep the look
sharp and modern rather than retro.

## Signature moment — Oscilloscope Sync

New focal component `OscilloscopeSync.vue`, canvas-rendered, driven by real store
data. It is the direct port of the current [glow computed in App.vue](../../../src/App.vue)
— same data in, waveform out — and sits where `ConvergenceStrip` currently lives
(the strip becomes the scope's readout row beneath it).

Data mapping (all already in the store):

- **Each panelist = one waveform channel.** Amplitude ∝ that seat's `confidence`
  (`live[seatId].confidence`). While `status === "streaming"` the trace is
  searching / jittery; on the `stance` event it snaps to its channel.
- **Phase grouping by stance cluster** — seats sharing the majority stance
  phase-align; outliers ride out of phase. (Cluster inferred from per-seat
  `stance` strings and the global `clusterFraction`.)
- **`clusterFraction`** → how many traces fold into one composite line.
- **`meanConfidence`** → composite amplitude + line cleanliness.
- **States (from `effectiveRuling`):**
  - `CONSENSUS` → one clean, bright-green locked sine + "SIGNAL LOCK" readout.
  - `CONTINUE` → partial alignment, dim green.
  - `DEADLOCK` → red desync noise, jitter, glitch.

Respects `--effects-intensity` (calmer trace / lower frame work at `ambient`, off
at `off`) and hooks the existing rAF perf auto-reduce so it degrades gracefully.

## Component reskin (structure unchanged, surface only)

- **`.terminal-panel`** → notched HUD panel with corner brackets and a phosphor
  edge (replaces the warm gradient + soft shadow).
- **`SeatCard`** → a **channel strip**: bracket-labeled id, Krypton telemetry,
  confidence rendered as a signal meter, glow tinted by its stance vs. the cluster.
- **`ConvergenceStrip`** → readout row beneath the oscilloscope.
- **`CockpitStatusBar`** → HUD top rail with a telemetry ticker; keeps the
  `off / ambient / max` effects toggle.
- **Boot overlay** (in App.vue) → handshake / boot sequence.
- **`VerdictScreen`** → "DECODED TRANSMISSION / RECORD": Departure Mono title,
  Xenon body, green frame when converged / red frame at deadlock; export path
  unchanged.
- **`.markdown-body`** → phosphor-green prose set in Argon; code in Xenon; green
  bullets and rules.

## Scope & files

Touched:

- [src/style.css](../../../src/style.css) — tokens, effects, panels, markdown (bulk of the work).
- [.impeccable.md](../../../.impeccable.md) — full rewrite of the design system doc.
- `brass → signal` rename sweep across `src/components/**` and `src/screens/**`.
- [src/App.vue](../../../src/App.vue) — boot overlay → handshake; the
  `.chamber-glow` radial layer is retired and replaced by the grid/dataflow layer.
  The `glow` computed's convergence→state logic moves into the oscilloscope (which
  reads the same `store.convergence`); the `--glow-hue`/`--glow-intensity` CSS
  vars and the radial gradient are removed.
- New `src/components/OscilloscopeSync.vue`.
- `src/components/ConvergenceStrip.vue` — demoted to readout row.
- Light per-component class tweaks (SeatCard, CockpitStatusBar, VerdictScreen,
  MediatorPanel, etc.).

**Not touched:** the Rust backend (`crates/`, `src-tauri/`), the wire types in
[src/lib/types.ts](../../../src/lib/types.ts), all store logic, screen structure /
layout, and keyboard shortcuts.

## Verification

- Drive the dev preview via `?preview=stream` (and other seed kinds) to watch the
  oscilloscope against seeded convergence telemetry through CONTINUE → CONSENSUS
  and → DEADLOCK.
- Confirm `effects=off` kills all motion (grid, dataflow, glow, oscilloscope
  animation, glitch).
- Confirm `prefers-reduced-motion` is respected (instant tokens, no animation).
- Confirm the perf auto-reduce loop still trips under load (oscilloscope hooks it).
- Screenshot proof in the real app across setup / room / verdict.

## Risks & tradeoffs

- **The oscilloscope is the one real build** (canvas + rAF) — most of the effort
  and the main perf watch item; it must hook the existing auto-reduce loop rather
  than run an independent uncapped animation.
- **Pure-black + neon can fatigue** — mitigated by keeping surfaces slightly
  lifted and gating glow on `--effects-intensity` (calm at `ambient`, loud at
  `max`).
- **Departure Mono is pixel type** — used at large sizes only; never as body text.
- The `brass → signal` rename is mechanical but wide; needs a careful sweep so no
  stray `brass` class or token survives.
