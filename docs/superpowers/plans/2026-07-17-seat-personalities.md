# Seat Personalities Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give each seat a composable, selectable personality (temperament + domain expert for panelists, a mediator-specific set for the foreman) from a baked-in catalog, plus an always-available custom free-text addendum.

**Architecture:** A frontend persona catalog (`src/lib/personas.ts`) holds all persona definitions and a pure `resolveSystemPrompt` function. Seats store an ordered `personas: string[]` of ids; the store resolves ids → concatenated prompt text into the existing `system_prompt` at `buildConfig()`, so the Rust engine is unchanged except for one additive `#[serde(default)]` field on `SeatConfig` (for deserialization + audit-snapshot recording). The `SeatEditor` UI gains grouped Selects bound through computed proxies over the array.

**Tech Stack:** Vue 3 (`<script setup lang="ts">`), Pinia, Tailwind v4, reka-ui/shadcn-vue Select, Rust (serde), Tauri.

## Global Constraints

- Engine reads `system_prompt` ONLY; it must not read `personas`. No deliberation-logic change.
- Rust `SeatConfig` addition must be `#[serde(default)]` — backward-compatible with existing stored configs (struct has no `deny_unknown_fields`).
- Persona prompt text: second person, ~1–3 sentences, NO output-format instructions (engine injects the stance/`agree_with` JSON structure).
- Personas never modify sampling params.
- reka-ui `SelectItem` cannot take an empty-string `value` — use the sentinel `"__none__"` for the "none" option.
- No frontend test runner exists: Rust task is TDD via `cargo test`; frontend tasks verified by `npm run build` (vue-tsc type-check) + browser walkthrough.
- Terminal-cockpit styling: match the existing dense `SeatEditor` (mono `text-[9px]`/`text-[10px]` labels, `bg-surface` inputs).

---

### Task 1: Additive `personas` field on Rust `SeatConfig`

**Files:**
- Modify: `crates/krunch-core/src/config.rs:66-77` (struct) and the test `SeatConfig` literal (~line 268)
- Test: `crates/krunch-core/src/config.rs` (inline `#[cfg(test)]` module)

**Interfaces:**
- Produces: `SeatConfig.personas: Vec<String>` (serde `default`), present in the wire config and audit snapshot. Consumed by no engine code.

- [ ] **Step 1: Write the failing test** — add to the `#[cfg(test)]` module in `crates/krunch-core/src/config.rs`:

```rust
#[test]
fn seat_config_personas_defaults_when_absent() {
    // Existing stored configs have no `personas` key — must default to empty.
    let json = r#"{
        "id": "00000000-0000-0000-0000-000000000001",
        "display_name": "Juror",
        "provider": "demo",
        "base_url": "",
        "model": "demo",
        "system_prompt": "you are a juror",
        "credential_ref": "",
        "role": "panelist"
    }"#;
    let seat: SeatConfig = serde_json::from_str(json).unwrap();
    assert!(seat.personas.is_empty());
}

#[test]
fn seat_config_personas_roundtrip() {
    let json = r#"{
        "id": "00000000-0000-0000-0000-000000000001",
        "display_name": "Juror",
        "provider": "demo",
        "base_url": "",
        "model": "demo",
        "system_prompt": "resolved text",
        "credential_ref": "",
        "role": "panelist",
        "personas": ["temp.skeptic", "dom.engineer"]
    }"#;
    let seat: SeatConfig = serde_json::from_str(json).unwrap();
    assert_eq!(seat.personas, vec!["temp.skeptic".to_string(), "dom.engineer".to_string()]);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p krunch-core personas`
Expected: FAIL — `no field 'personas' on type SeatConfig` (compile error).

- [ ] **Step 3: Add the field** — in `crates/krunch-core/src/config.rs`, inside `struct SeatConfig`, after the `sampling` field:

```rust
    #[serde(default)]
    pub sampling: SamplingParams,
    /// Ordered persona ids (frontend catalog). Recorded for the audit snapshot;
    /// the engine does not read these — it consumes `system_prompt` only.
    #[serde(default)]
    pub personas: Vec<String>,
    pub credential_ref: String,
    pub role: Role,
```

- [ ] **Step 4: Fix the explicit test literal** — the `SeatConfig { ... }` literal near line 268 now needs the field. Add `personas: vec![],` alongside its other fields (place it after `sampling: ...`). If any other `SeatConfig { .. }` literals exist in the crate, add `personas: vec![]` to each (grep `SeatConfig {` to find them).

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p krunch-core`
Expected: PASS (all, including the two new tests).

- [ ] **Step 6: Commit**

```bash
git add crates/krunch-core/src/config.rs
git commit -m "feat(core): add additive personas field to SeatConfig"
```

---

### Task 2: Persona catalog + pure resolver (`src/lib/personas.ts`)

**Files:**
- Create: `src/lib/personas.ts`

**Interfaces:**
- Produces:
  - `type PersonaGroup = "temperament" | "domain" | "mediator"`
  - `interface Persona { id: string; label: string; group: PersonaGroup; prompt: string }`
  - `const PERSONAS: Persona[]`
  - `personaById(id: string): Persona | undefined`
  - `personasForGroup(group: PersonaGroup): Persona[]`
  - `groupsForRole(role: "panelist" | "mediator"): PersonaGroup[]` → panelist: `["temperament","domain"]`; mediator: `["mediator"]`
  - `resolveSystemPrompt(personaIds: string[], addendum: string): string`
  - `const NONE = "__none__"` (Select sentinel for "no persona in this group")

- [ ] **Step 1: Create the catalog file** — `src/lib/personas.ts`:

```ts
export type PersonaGroup = "temperament" | "domain" | "mediator";

export interface Persona {
  id: string;
  label: string;
  group: PersonaGroup;
  prompt: string;
}

/** Sentinel value for the "no persona" option (reka SelectItem forbids ""). */
export const NONE = "__none__";

// Canonical concatenation order when resolving a system prompt.
const GROUP_ORDER: PersonaGroup[] = ["temperament", "domain", "mediator"];

export const PERSONAS: Persona[] = [
  // — Temperaments —
  { id: "temp.skeptic", label: "Skeptic", group: "temperament", prompt: "You are the Skeptic. You distrust unsupported claims and press hard for evidence, clear definitions, and sound reasoning before you agree to anything." },
  { id: "temp.optimist", label: "Optimist", group: "temperament", prompt: "You are the Optimist. You look for what could go right, surface the upside and best-case paths, and argue for ambition over excessive caution." },
  { id: "temp.pragmatist", label: "Pragmatist", group: "temperament", prompt: "You are the Pragmatist. You care about what can actually be done under real constraints of time, effort, and resources, and steer toward concrete, workable options." },
  { id: "temp.devils_advocate", label: "Devil's Advocate", group: "temperament", prompt: "You are the Devil's Advocate. Whatever position is gaining favor, you argue the strongest case against it and expose the assumptions the group is taking for granted." },
  { id: "temp.first_principles", label: "First-Principles", group: "temperament", prompt: "You are the First-Principles thinker. You strip problems down to fundamentals and reason up from them, distrusting convention, analogy, and how things are usually done." },
  { id: "temp.risk_hawk", label: "Risk-Hawk", group: "temperament", prompt: "You are the Risk-Hawk. You hunt for failure modes, downside scenarios, and tail risks, and you insist the panel reckon with what happens when things go wrong." },
  { id: "temp.synthesizer", label: "Synthesizer", group: "temperament", prompt: "You are the Synthesizer. You look for the common ground beneath disagreement, integrate the strongest parts of each view, and propose positions the panel can converge on." },
  { id: "temp.contrarian", label: "Contrarian", group: "temperament", prompt: "You are the Contrarian. You resist easy agreement and challenge the emerging consensus, forcing the panel to earn its conclusions." },
  // — Domain experts —
  { id: "dom.engineer", label: "Engineer", group: "domain", prompt: "You reason as an Engineer. You weigh feasibility, systems constraints, failure surfaces, and concrete implementation tradeoffs." },
  { id: "dom.lawyer", label: "Lawyer", group: "domain", prompt: "You reason as a Lawyer. You weigh liability, compliance, rights, precedent, and how commitments would hold up under scrutiny." },
  { id: "dom.ethicist", label: "Ethicist", group: "domain", prompt: "You reason as an Ethicist. You weigh harms and benefits, fairness, consent, and duties to those affected." },
  { id: "dom.economist", label: "Economist", group: "domain", prompt: "You reason as an Economist. You weigh incentives, costs and benefits, opportunity cost, and second-order effects." },
  { id: "dom.scientist", label: "Scientist", group: "domain", prompt: "You reason as a Scientist. You frame claims as hypotheses, ask what evidence would confirm or falsify them, and distrust conclusions that outrun the data." },
  { id: "dom.designer", label: "Designer", group: "domain", prompt: "You reason as a Designer. You start from the people affected, their needs and experience, and argue for clarity and simplicity." },
  { id: "dom.historian", label: "Historian", group: "domain", prompt: "You reason as a Historian. You look for precedent and pattern, ask when this has been tried before, and what happened when it was." },
  { id: "dom.security", label: "Security Analyst", group: "domain", prompt: "You reason as a Security Analyst. You think in threat models, abuse cases, and worst-case adversaries, and ask how this could be exploited." },
  // — Mediator —
  { id: "med.neutral_foreman", label: "Neutral Foreman", group: "mediator", prompt: "You are a neutral foreman. You stay impartial, structure the discussion fairly, and synthesize the panel's reasoning without taking a side." },
  { id: "med.strict_timekeeper", label: "Strict Timekeeper", group: "mediator", prompt: "You are a strict timekeeper. You keep the panel moving toward a decision, discourage drift and repetition, and press for closure." },
  { id: "med.consensus_seeker", label: "Consensus-Seeker", group: "mediator", prompt: "You are a consensus-seeker. You actively look for bridges between positions and steer the panel toward agreement it can genuinely hold." },
  { id: "med.socratic_chair", label: "Socratic Chair", group: "mediator", prompt: "You are a Socratic chair. You draw out the panel's reasoning with probing questions rather than asserting conclusions yourself." },
];

const BY_ID = new Map(PERSONAS.map((p) => [p.id, p]));

export function personaById(id: string): Persona | undefined {
  return BY_ID.get(id);
}

export function personasForGroup(group: PersonaGroup): Persona[] {
  return PERSONAS.filter((p) => p.group === group);
}

export function groupsForRole(role: "panelist" | "mediator"): PersonaGroup[] {
  return role === "mediator" ? ["mediator"] : ["temperament", "domain"];
}

/**
 * Resolve persona ids + a free-text addendum into a single system prompt.
 * Order is canonical (temperament → domain → mediator); unknown ids are
 * skipped; the trimmed addendum is appended last; fragments join with a blank
 * line. Everything empty → "" (no system prompt).
 */
export function resolveSystemPrompt(personaIds: string[], addendum: string): string {
  const fragments = [...personaIds]
    .map((id) => personaById(id))
    .filter((p): p is Persona => Boolean(p))
    .sort((a, b) => GROUP_ORDER.indexOf(a.group) - GROUP_ORDER.indexOf(b.group))
    .map((p) => p.prompt);
  const trimmed = addendum.trim();
  if (trimmed) fragments.push(trimmed);
  return fragments.join("\n\n");
}
```

- [ ] **Step 2: Type-check** — `resolveSystemPrompt` is the pure unit under test; the repo has no JS runner, so verify by compilation:

Run: `npx vue-tsc --noEmit`
Expected: no errors.

- [ ] **Step 3: Verify resolver behavior by inspection** (documented cases — the implementer confirms each holds for the code above; these become runnable tests if a JS runner is later added):

| `personaIds` | `addendum` | expected `resolveSystemPrompt` |
|---|---|---|
| `[]` | `""` | `""` |
| `["temp.skeptic"]` | `""` | Skeptic prompt |
| `["dom.engineer"]` | `""` | Engineer prompt |
| `["dom.engineer","temp.skeptic"]` | `""` | Skeptic prompt + `\n\n` + Engineer prompt (temperament first) |
| `["temp.skeptic"]` | `"  focus on X  "` | Skeptic prompt + `\n\n` + `focus on X` |
| `[]` | `"only my text"` | `only my text` |
| `["bogus.id"]` | `""` | `""` (unknown skipped) |

- [ ] **Step 4: Commit**

```bash
git add src/lib/personas.ts
git commit -m "feat(ui): add persona catalog and pure system-prompt resolver"
```

---

### Task 3: Wire personas into the store

**Files:**
- Modify: `src/lib/types.ts:41-51` (`SeatConfig` interface)
- Modify: `src/stores/deliberation.ts` (`newSeat`, `buildConfig`, `loadDemoPanel`)

**Interfaces:**
- Consumes: `resolveSystemPrompt` (Task 2), `SeatConfig.personas` (Task 1 wire shape).
- Produces: every seat carries `personas: string[]`; `buildConfig()` sends a resolved `system_prompt` per seat.

- [ ] **Step 1: Add `personas` to the TS `SeatConfig`** — in `src/lib/types.ts`, inside `interface SeatConfig`, after `sampling`:

```ts
  sampling: SamplingParams;
  personas: string[];
  credential_ref: string;
  role: Role;
```

- [ ] **Step 2: Default `personas` in `newSeat`** — in `src/stores/deliberation.ts`, in the `newSeat` return object, add `personas: []` (before `role`):

```ts
    system_prompt: "", sampling: { temperature: 0.7 }, personas: [], credential_ref: "anthropic-default", role, ...partial,
```

- [ ] **Step 3: Resolve personas in `buildConfig`** — import the resolver at the top of `src/stores/deliberation.ts`:

```ts
import { resolveSystemPrompt } from "@/lib/personas";
```

Then rewrite `buildConfig` so each seat's `system_prompt` is the resolved text (personas + the seat's own `system_prompt` as addendum), while `personas` still rides along:

```ts
  function buildConfig(): SessionConfig {
    const seatsResolved = seats.value.map((seat) => ({
      ...JSON.parse(JSON.stringify(seat)),
      system_prompt: resolveSystemPrompt(seat.personas, seat.system_prompt),
    }));
    return { problem: problem.value, mode: mode.value, max_rounds: maxRounds.value, guard: { quorum_fraction: quorumFraction.value, confidence_floor: confidenceFloor.value }, seats: seatsResolved };
  }
```

- [ ] **Step 4: Give the demo panel contrasting personas** — in `loadDemoPanel`, set `personas` on each demo seat:

```ts
  function loadDemoPanel() {
    const demo = (role: SeatConfig["role"], display_name: string, personas: string[]) => newSeat(role, { display_name, provider: "demo", base_url: "", model: "demo", credential_ref: "", personas });
    seats.value = [
      demo("mediator", "Foreman (demo)", ["med.neutral_foreman"]),
      demo("panelist", "Juror A (demo)", ["temp.optimist", "dom.designer"]),
      demo("panelist", "Juror B (demo)", ["temp.skeptic", "dom.engineer"]),
    ];
    if (!problem.value.trim()) problem.value = "Should our team adopt a four-day work week?";
  }
```

- [ ] **Step 5: Type-check**

Run: `npx vue-tsc --noEmit`
Expected: no errors. (If `SeatConfig` is constructed elsewhere without `personas`, the compiler flags it — add `personas: []` there.)

- [ ] **Step 6: Commit**

```bash
git add src/lib/types.ts src/stores/deliberation.ts
git commit -m "feat(ui): resolve seat personas into system_prompt at config build"
```

---

### Task 4: Persona pickers in `SeatEditor.vue`

**Files:**
- Modify: `src/components/SeatEditor.vue`

**Interfaces:**
- Consumes: `personasForGroup`, `personaById`, `groupsForRole`, `NONE` (Task 2); `seat.personas` (Task 3).

- [ ] **Step 1: Extend the script block** — in `src/components/SeatEditor.vue` `<script setup>`, add imports and per-group computed proxies + role reconcile:

```ts
import { computed, onMounted, ref, watch } from "vue";
import type { SeatConfig } from "@/lib/types";
import type { PersonaGroup } from "@/lib/personas";
import { NONE, personaById, personasForGroup, groupsForRole } from "@/lib/personas";
```

Add after the existing `props`/`http`/`needsKey` setup:

```ts
// One proxy per persona group: reads/writes the single id of that group inside
// seat.personas, using NONE as the "no persona" sentinel.
function groupProxy(group: PersonaGroup) {
  return computed<string>({
    get: () => props.seat.personas.find((id) => personaById(id)?.group === group) ?? NONE,
    set: (id: string) => {
      const others = props.seat.personas.filter((pid) => personaById(pid)?.group !== group);
      props.seat.personas = id === NONE ? others : [...others, id];
    },
  });
}
const temperament = groupProxy("temperament");
const domain = groupProxy("domain");
const mediatorPersona = groupProxy("mediator");

const temperamentOptions = personasForGroup("temperament");
const domainOptions = personasForGroup("domain");
const mediatorOptions = personasForGroup("mediator");

// When a seat's role changes, drop persona ids whose group is invalid for it.
watch(() => props.seat.role, (role) => {
  const allowed = groupsForRole(role);
  props.seat.personas = props.seat.personas.filter((id) => {
    const g = personaById(id)?.group;
    return g !== undefined && allowed.includes(g);
  });
});
```

- [ ] **Step 2: Replace the SYSTEM PROMPT label with persona selects + relabeled addendum** — in the template, replace the existing `<label class="col-span-2">SYSTEM PROMPT<Textarea .../></label>` block with:

```html
<template v-if="seat.role !== 'mediator'">
  <label>TEMPERAMENT<Select v-model="temperament"><SelectTrigger class="mt-1 h-8 bg-surface"><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem :value="NONE">— None —</SelectItem><SelectItem v-for="p in temperamentOptions" :key="p.id" :value="p.id">{{ p.label }}</SelectItem></SelectGroup></SelectContent></Select></label>
  <label>DOMAIN EXPERT<Select v-model="domain"><SelectTrigger class="mt-1 h-8 bg-surface"><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem :value="NONE">— None —</SelectItem><SelectItem v-for="p in domainOptions" :key="p.id" :value="p.id">{{ p.label }}</SelectItem></SelectGroup></SelectContent></Select></label>
</template>
<label v-else class="col-span-2">MEDIATOR PERSONA<Select v-model="mediatorPersona"><SelectTrigger class="mt-1 h-8 bg-surface"><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem :value="NONE">— None —</SelectItem><SelectItem v-for="p in mediatorOptions" :key="p.id" :value="p.id">{{ p.label }}</SelectItem></SelectGroup></SelectContent></Select></label>
<label class="col-span-2">CUSTOM (ADDENDUM)<Textarea v-model="seat.system_prompt" rows="2" placeholder="optional extra instructions, appended after the persona" class="mt-1 resize-y bg-surface text-[10px]" /></label>
```

(`Select`, `SelectContent`, `SelectGroup`, `SelectItem`, `SelectTrigger`, `SelectValue` are already imported in this file.)

- [ ] **Step 3: Type-check + build**

Run: `npm run build`
Expected: `vue-tsc` clean, `vite build` succeeds.

- [ ] **Step 4: Commit**

```bash
git add src/components/SeatEditor.vue
git commit -m "feat(ui): temperament/domain/mediator persona pickers in seat editor"
```

---

### Task 5: Full verification

**Files:** none (verification only).

- [ ] **Step 1: Proof build + tests**

Run: `npm run build && cargo test`
Expected: build clean; all cargo tests pass (including Task 1's two new tests).

- [ ] **Step 2: Browser walkthrough** — with the dev server on `localhost:1420`:
  - Open `/` (setup). On a panelist seat, pick TEMPERAMENT = Skeptic and DOMAIN EXPERT = Engineer; type text in CUSTOM (ADDENDUM).
  - Confirm the mediator seat shows only MEDIATOR PERSONA (no temperament/domain).
  - Click **Load demo panel**; confirm Juror A = Optimist/Designer, Juror B = Skeptic/Engineer, Foreman = Neutral Foreman are pre-selected.
  - Confirm no console errors (`read_console_messages`).

- [ ] **Step 3: Verify resolution reaches the engine** — run a demo deliberation (demo provider, no keys) and confirm it starts and streams without error, proving the resolved `system_prompt` payload is accepted by `start_deliberation`. (Optional deeper check: temporarily log `buildConfig()` output in dev and confirm a seat's `system_prompt` contains the composed persona text.)

- [ ] **Step 4: Final commit (if any verification fixes were needed)** — otherwise nothing to commit.

---

## Self-Review

**Spec coverage:**
- Data model (`personas: string[]` TS + Rust `#[serde(default)]`) → Tasks 1, 3. ✓
- Frontend catalog + pure resolver → Task 2. ✓
- Resolution at `buildConfig` (canonical order, addendum, empty→"") → Task 2 (fn) + Task 3 (call site). ✓
- UI: two panelist selects / one mediator select, relabeled addendum always visible, role reconcile → Task 4. ✓
- Demo panel contrasting personas → Task 3 Step 4. ✓
- Reproducibility (personas on wire/snapshot) → Task 1 (field) + Task 3 (rides along in `buildConfig`). ✓
- Persona authoring rules (no format instructions, composes) → Task 2 catalog content. ✓
- Testing (Rust deserialization + default; frontend resolver cases + browser) → Tasks 1, 2, 5. ✓
- Non-goals (no sampling coupling, no catalog editing, no per-round switch, no engine logic change) → respected across tasks. ✓

**Placeholder scan:** No TBD/TODO; all code shown in full; resolver cases enumerated with expected outputs.

**Type consistency:** `resolveSystemPrompt(personaIds, addendum)`, `personaById`, `personasForGroup`, `groupsForRole`, `NONE`, `PersonaGroup`, `Persona` used consistently across Tasks 2/3/4. `SeatConfig.personas: string[]` (TS) ↔ `Vec<String>` (Rust) aligned.
