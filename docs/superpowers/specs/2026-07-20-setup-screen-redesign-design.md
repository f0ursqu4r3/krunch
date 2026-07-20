# Setup screen redesign — "The Bench"

**Date:** 2026-07-20
**Screen:** `src/screens/SetupScreen.vue` + `src/components/SeatEditor.vue`
**Status:** approved direction, spec for review

## Problem

The "Convene the panel" setup screen doesn't hold together. Four concrete failures,
all confirmed with the user:

1. **Density / repetition.** Every seat renders all ~10 config fields at once
   (provider, model, base URL, two persona selects, addendum, temperature, top-p,
   max tokens, credential ref, API key). N seats = a wall of near-identical forms.
   The essential choice — *who is on the panel* — drowns under knobs set once or never.
2. **Balance / whitespace.** A 2-column seat grid with an odd seat count strands the
   3rd card alone with a large dead void beside it. Cards are unequal height, so rows
   never align. The readiness sidebar is a thin floating strip with empty space below.
3. **Weak primary action.** Convening is the entire point, but the CTA is a small
   button in the floating sidebar — easy to lose.
4. **Flow.** Matter / rules / panel read as three disconnected slabs, not one
   coherent "seat the panel, then open deliberation" ritual.

## Direction: master-detail ("The Bench")

Approved via mockup (`docs/superpowers/specs/assets` not required — mockup was a
throwaway artifact). The screen becomes:

```
┌─ top bar (unchanged) ─────────────────────────────────────────────┐
│ Convene the panel  ·  State the matter, seat the panel…            │
├──────────────────────────────────────┬────────────────────────────┤
│ THE MATTER (hero textarea)           │ READINESS + [Convene panel] │  ← hero row
├──────────────────────────────────────┴────────────────────────────┤
│ RULES strip: mode · max rounds · quorum · confidence (thin)        │
├──────────────────────┬─────────────────────────────────────────────┤
│ THE PANEL // n/6      │  EDITING: <seat name>                        │
│ ┌──────────────────┐ │  provider   model                            │  ← panel region
│ │ • Mediator  MED  │ │  persona(s)                                   │    (master │ detail)
│ │   Panelist 2     │ │  custom addendum                             │
│ │   Panelist 3     │ │  › Advanced (sampling, base URL, credentials)│
│ │ + Add seat       │ │                                              │
│ └──────────────────┘ │                                              │
└──────────────────────┴─────────────────────────────────────────────┘
   ▸ sticky convene bar appears at the bottom once the hero scrolls out of view
```

**Reading order** is the ritual: state the matter → see who's on the bench → convene.
Rules are demoted to a thin strip because they are set once.

## Components

Refactor the two current monolith components into four focused units.

### `SetupScreen.vue` (orchestrator)
Owns layout and selection state. Sections: header, hero (matter + `ConveneCard`),
rules strip, panel region (`SeatRoster` + `SeatEditor`), and a bottom `ConveneBar`.

- Local `selectedSeatId = ref<string>()`, defaulting to the mediator's id.
- On add seat → select the new seat. On remove of the selected seat → fall back to
  the mediator (always present, so no empty-selection state).
- Rules strip is a compact inline row (mode toggle + three numeric fields), same
  bindings as today (`store.mode`, `maxRounds`, `quorumDisplay`, `confidenceFloor`).

### `SeatRoster` (new — master list)
A vertical list: mediator first, then panelists, then an "+ Add seat" ghost row
(disabled at 6 seats, keeps the `A` shortcut hint). Each row is a `SeatRosterItem`.
- Props: `seats`, `selectedId`. Emits `select(id)`, `add`.
- Selected row: brass left-edge + raised surface. A list cannot strand like the grid.

### `SeatRosterItem` (new — one row)
Compact, read-only summary of a seat — never a form.
- Role badge (`MED` brass / `SEAT`), display name (serif).
- Persona chips derived from `seat.personas` via `personaById(...).label`.
- `provider · model` in mono (or `demo` / `claude cli` label for CLI/demo providers).
- A subtle warning affordance (e.g. a dim dot or `key?` chip) when the seat needs a
  credential that isn't stored — so hiding the key field under Advanced never hides a
  blocking problem. Derivation only; the blocking copy still lives in readiness.

### `SeatEditor.vue` (refactor — detail pane)
Edits the one selected seat. Same field set and same `groupProxy` persona logic as
today, reorganized into:
- **Header:** role badge + editable display name (name editing moves here from the
  roster) + `remove` (panelists only).
- **Essentials (always visible):** provider, model (hidden for demo), persona
  select(s) — temperament + domain for panelists, mediator persona for the mediator —
  and the custom addendum.
- **Advanced (collapsed by default):** base URL (http providers), temperature, top-p,
  max tokens, credential ref + API key (when `needsKey`). Local `showAdvanced` ref.
  The provider/demo/CLI helper notes stay near provider.

### `ConvenePanel` (new — shared convene surface, one component, two placements)
One presentational component rendering readiness + the Convene action, taking a
`variant: "card" | "bar"` prop, rendered twice by `SetupScreen`:
1. **Hero card** (`variant="card"`, top-right, visible on load) — the strong first
   impression.
2. **Sticky bottom bar** (`variant="bar"`, full width) — appears once the hero card
   scrolls out of view, so Convene is always reachable while working the roster.

- Readiness summary: `1 med · n seats · <mode> · max <rounds>`, plus the validation
  list (`[✓] ready to convene` or one `[!] …` per `store.validation` item) and
  `store.startError`.
- Convene button disabled while `store.validation.length > 0`; calls `store.start()`;
  keeps the `C` shortcut.
- Reveal driven by `useIntersectionObserver` (from `@vueuse/core`, already a dep) on
  the hero card. Reduced-motion: fade only.

## State & store

No store shape changes required. `selectedSeatId` is local UI state in
`SetupScreen`. Existing store surface is reused as-is: `problem`, `mode`, `maxRounds`,
`quorumFraction`, `confidenceFloor`, `mediator`, `panelists`, `validation`,
`startError`, `start()`, `addPanelist()`, `removeSeat()`, `loadDemoPanel()`.

## Behavior details

- **Default selection:** mediator on mount; there is always exactly one mediator.
- **Add seat:** `store.addPanelist()` then select the returned/new seat's id.
- **Remove seat:** if removing the selected seat, reselect mediator; then
  `store.removeSeat(id)`.
- **Persona chips:** derived read-only from `seat.personas`; empty personas → no chips
  (row still shows provider·model).
- **Keyboard:** keep `A` (add) and `C` (convene). Optional, not required: ↑/↓ move
  roster selection when the roster is focused — document as a nice-to-have, not in scope.
- **Responsive:** desktop Tauri target. Below ~1000px, hero and panel region collapse
  to single column (roster above editor). No mobile layout required.

## Aesthetic

Unchanged design system — the warm chamber (`src/style.css`): brass accent, Young
Serif display, Monaspace mono labels, `.terminal-panel` furniture, the convergence
glow. This is a layout re-composition, not a re-skin. Single dark theme is a
deliberate, existing choice.

## Carrying the pattern to the Room (documentation only — not built here)

The bench pattern should later echo into `RoomScreen.vue`: a compact **seat rail**
(the roster's live sibling — each panelist as a row showing live stance/status/token
counts) beside a **focused transcript** for the selected seat, instead of showing every
seat's full stream with equal weight. This keeps a single mental model across setup and
deliberation: *the panel is a list of seats; you focus one at a time.* Out of scope for
this change; captured so the setup work is built with the Room in mind (shared
`SeatRosterItem`-style summary, shared chip/badge styling).

## Testing & verification

- **Unit (Vitest-style logic, TDD where it pays):** persona-chip derivation from
  `seat.personas`; selection fallback when the selected seat is removed; readiness
  summary string. (Note: repo has no test runner yet — if adding one is out of scope,
  these become manual checks and are called out in the plan.)
- **Visual / interaction:** verify in the dev server via the browser preview workflow —
  roster select swaps the editor, Advanced expander toggles, sticky bar reveals on
  scroll, Convene enables/disables with validation, demo panel loads and renders,
  glow/atmosphere intact.

## Out of scope

- RoomScreen / VerdictScreen changes (Room pattern is documented, not built).
- Store/data-model changes.
- New credential or provider features.
- A test-runner setup, unless the plan explicitly opts in.
