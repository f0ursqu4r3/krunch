# Design: Selectable seat personalities

_Date: 2026-07-17 · krunch · brainstormed with kyle.dougan_

## Goal

Give each seat a **selectable personality** from a baked-in catalog, with a
**Custom** free-text option. Personalities shape *how* a seat argues (voice /
stance / lens) without touching krunch's deliberation machinery. Panelists can
**compose** a temperament with a domain expert (e.g. "Skeptic Engineer"); the
mediator picks from a mediator-specific set. All layers are optional and
additive — a seat can be pure-preset, preset + custom notes, or pure-custom.

## Data model

`SeatConfig` gains one field:

```ts
personas: string[]   // ordered persona ids; e.g. ["temp.skeptic", "dom.engineer"]
```

- **TypeScript** (`src/lib/types.ts`): add `personas: string[]` to `SeatConfig`.
- **Rust** (`crates/krunch-core/src/config.rs:66`, `struct SeatConfig`): add
  `#[serde(default)] pub personas: Vec<String>`. The struct has no
  `deny_unknown_fields` and already uses `#[serde(default)]` on `sampling`, so
  this is backward-compatible with existing stored configs and deserializes the
  frontend payload; the ids are recorded in the audit snapshot. The explicit
  `SeatConfig` literal in that file's tests (~line 268) must add the field.
  **The engine does not read `personas`** — it continues to consume
  `system_prompt` only.
- `system_prompt` is retained unchanged as the **custom addendum** (free text).

Panelist `personas` holds at most one `temperament`-group id and at most one
`domain`-group id. Mediator `personas` holds at most one `mediator`-group id.
The UI enforces these caps; the resolver tolerates any order.

## The catalog

`src/lib/personas.ts` exports a flat list of persona definitions:

```ts
export interface Persona { id: string; label: string; group: PersonaGroup; prompt: string }
export type PersonaGroup = "temperament" | "domain" | "mediator";
```

Roster (~20):

- **temperament (8):** Skeptic, Optimist, Pragmatist, Devil's Advocate,
  First-Principles, Risk-Hawk, Synthesizer, Contrarian.
- **domain (8):** Engineer, Lawyer, Ethicist, Economist, Scientist, Designer,
  Historian, Security Analyst.
- **mediator (4):** Neutral Foreman, Strict Timekeeper, Consensus-Seeker,
  Socratic Chair.

Ids are namespaced by group (`temp.*`, `dom.*`, `med.*`) so role-validity and
grouping are derivable from the id.

### Persona prompt authoring rules

Each `prompt` is a short **voice/stance overlay** (~1–3 sentences), written so
that:

1. It contains **no output-format instructions** — krunch's engine already
   injects the stance / `agree_with` JSON structure via its round prompts.
   Personas that fought that structure would break parsing.
2. It **composes gracefully**: a temperament fragment and a domain fragment must
   read naturally when concatenated (each self-contained, additive, second
   person, no contradictory framing). Example resolved text:
   > You are the Skeptic: you distrust unsupported claims and press hard for
   > evidence before agreeing.
   >
   > You reason as an Engineer: you weigh feasibility, systems constraints, and
   > concrete tradeoffs.

## Resolution (pure function)

`personas.ts` exports:

```ts
resolveSystemPrompt(personaIds: string[], addendum: string): string
```

Behavior: look up each id's prompt, order canonically
(temperament → domain → mediator), append the trimmed `addendum` if non-empty,
join non-empty fragments with a blank line. All empty → `""` (today's behavior:
no system prompt). Unknown ids are skipped (defensive against stale audit
configs). This is called in the store's `buildConfig()` to produce each seat's
`system_prompt` before the config is sent to `start_deliberation`.

Keeping resolution a pure, exported function makes it unit-testable in isolation
and keeps `buildConfig` thin.

## UI (`SeatEditor.vue`)

- **Panelist seat:** two grouped `Select`s reusing the existing shadcn Select —
  **TEMPERAMENT** (8 options + "— None —") and **DOMAIN EXPERT** (8 + "— None —").
  Below them, the existing free-text field, relabeled **CUSTOM (ADDENDUM)**,
  always visible.
- **Mediator seat:** one **MEDIATOR PERSONA** Select (4 + "— None —") + the
  CUSTOM (ADDENDUM) field.
- Selects bind through computed proxies over `seat.personas` (get = find id in
  group; set = replace that group's slot in the array, dropping it on "None").
- **Role reconcile:** when a seat's role changes, drop persona ids whose group is
  invalid for the new role (a panelist's temperament/domain don't carry to
  mediator, and vice-versa).
- Terminal-cockpit styling consistent with the rest of the dense setup pane.

## Demo panel

`loadDemoPanel()` seats contrasting personas for a livelier out-of-the-box demo,
e.g. panelist A = Optimist · Designer, panelist B = Skeptic · Engineer, mediator
= Neutral Foreman. Falls back to the existing demo problem text.

## Reproducibility

`personas` rides along on the wire config and is persisted in the audit snapshot,
so a filed session records *which* personas ran (not just the resolved prose).
The resolved `system_prompt` is what the model actually received and is stored as
today.

## Testing

- **Resolver:** `resolveSystemPrompt` is pure — enumerate cases (none, temperament
  only, domain only, both, both + addendum, addendum only, unknown id skipped,
  canonical ordering). The repo has **no frontend test runner** (established in the
  prior build); document these cases and rely on type-check + browser verification
  until a runner exists.
- **Rust:** a deserialization test that a `SeatConfig` JSON payload carrying
  `personas` round-trips, and that omitting `personas` defaults to empty (serde
  default) — proving backward compatibility with existing stored configs.
- **Browser:** in setup, pick a temperament + domain on a panelist, confirm the
  CUSTOM addendum composes, load the demo panel and confirm contrasting personas,
  switch a seat's role and confirm invalid personas drop.

## Non-goals

- Personas do **not** modify sampling parameters (temperature/top_p/max_tokens) —
  those stay under separate user control.
- No editing/saving custom text back into the baked-in catalog (Custom is
  per-seat free text only).
- No per-round or mid-session persona switching — personas are set at config time.
- No engine/deliberation-logic changes beyond the additive `personas` field.
