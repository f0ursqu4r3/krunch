# Setup screen redesign ("The Bench") Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild the "Convene the panel" setup screen as a master-detail "bench" — a scannable seat roster + a single detail editor with progressive disclosure, a hero matter + convene action, and a sticky convene bar.

**Architecture:** `SetupScreen.vue` becomes a thin orchestrator holding `selectedSeatId` and layout. Seats render as a compact `SeatRoster`/`SeatRosterItem` list (master); the selected seat opens in a refactored `SeatEditor` (detail) whose sampling/base-URL/credential fields fold under an "Advanced" disclosure. A single `ConvenePanel` component renders readiness + the Convene action twice — as the hero card and as a sticky bottom bar revealed on scroll.

**Tech Stack:** Vue 3.5 `<script setup>`, Pinia, Tailwind v4, reka-ui (shadcn-vue primitives), `@vueuse/core` (`useIntersectionObserver`), `@lucide/vue` icons.

## Verification approach (read first)

The repo has **no test runner** (no `test` script, no vitest) and this change is
presentational. Per the approved spec, we do **not** add a runner. Every task's gate is:

1. **Type check:** `npx vue-tsc --noEmit` → expect no errors.
2. **Visual/interaction check** in the Browser pane dev server (setup is the default
   screen — no `?preview=` needed). Use the standard preview verification workflow:
   `preview_start` the dev server, `read_page`/`read_console_messages` to confirm
   structure and no errors, `computer` clicks to exercise interactions, `screenshot`
   as proof.

This deviates from strict TDD deliberately: no runner exists, the logic surface is
one pure helper, and the value is visual. The one pure helper (`personaLabels`) is
written to be trivially checkable.

If `.claude/launch.json` has no dev entry, create one before Task 1:

```json
{
  "version": "0.0.1",
  "configurations": [
    { "name": "krunch-dev", "runtimeExecutable": "npm", "runtimeArgs": ["run", "dev"], "port": 5173 }
  ]
}
```

## Global Constraints

- **Design system is fixed** — the warm chamber in `src/style.css`: brass accent,
  `.terminal-panel` furniture, `font-display` (Young Serif), `font-mono` (Monaspace),
  the convergence glow. This is a re-composition, not a re-skin. Single dark theme is
  an existing deliberate choice — do not add a light theme.
- **Label casing follows the existing screen** — uppercase mono eyebrows/labels
  (e.g. `INTERACTION MODE`, `PROVIDER`). Do not "correct" to sentence case.
- **No store shape changes** — reuse `useDeliberation()` as-is: `problem`, `mode`,
  `maxRounds`, `quorumFraction`, `confidenceFloor`, `seats`, `panelists`, `mediator`,
  `validation`, `startError`, `start()`, `addPanelist()`, `removeSeat(id)`,
  `loadDemoPanel()`.
- **Keep keyboard shortcuts** — `A` (add seat) and `C` (convene) are handled in
  `App.vue` and call the store directly; selection state must react to store changes,
  not rely on component click handlers.
- **Desktop Tauri target** — no mobile layout required, but collapse to a single
  column below ~1000px (`lg:` breakpoint boundary) so nothing overflows.
- **Vue 3.5 `<script setup lang="ts">`**, `@/` path alias, `@vueuse/core` ^11.

---

### Task 1: `personaLabels` helper

The one shared pure function — turns a seat's persona ids into display labels for the
roster chips.

**Files:**
- Modify: `src/lib/personas.ts` (append one exported function after `personaById`)

**Interfaces:**
- Consumes: existing `personaById(id): Persona | undefined`
- Produces: `personaLabels(ids: string[]): string[]` — labels in array order, unknown ids dropped

- [ ] **Step 1: Add the helper**

Append to `src/lib/personas.ts` (after the `personaById` function, around line 46):

```ts
/** Human-readable labels for a seat's persona ids, in array order; unknown ids dropped. */
export function personaLabels(ids: string[]): string[] {
  return ids
    .map((id) => personaById(id)?.label)
    .filter((label): label is string => Boolean(label));
}
```

- [ ] **Step 2: Type check**

Run: `npx vue-tsc --noEmit`
Expected: no errors.

- [ ] **Step 3: Sanity-check the logic (optional, no runner)**

Run: `npx tsx -e "import('./src/lib/personas.ts').then(m => console.log(m.personaLabels(['temp.skeptic','dom.security','bogus.id'])))"`
Expected: `[ 'Skeptic', 'Security Analyst' ]` (if `tsx` is unavailable, skip — the type check plus Task 3's visual check cover it).

- [ ] **Step 4: Commit**

```bash
git add src/lib/personas.ts
git commit -m "feat(setup): personaLabels helper for roster chips"
```

---

### Task 2: `SeatEditor.vue` — detail pane with Advanced disclosure

Refactor the existing all-fields-at-once editor into essentials + a collapsible
Advanced section. This task does **not** touch `SetupScreen.vue`; the editor still
renders per-seat in the current grid, so it is independently verifiable — every card
now shows ~4 fields with an "Advanced" toggle.

**Files:**
- Modify (full rewrite): `src/components/SeatEditor.vue`

**Interfaces:**
- Consumes: `SeatConfig`, `providerIsHttp`, `isLoopbackUrl` (`@/lib/types`);
  `NONE`, `personaById`, `personasForGroup`, `groupsForRole` (`@/lib/personas`);
  `api.hasCredential`, `api.setCredential` (`@/lib/api`)
- Produces: component with props `{ seat: SeatConfig; removable?: boolean }`,
  emit `remove` — unchanged signature, so `SetupScreen` needs no change this task.

- [ ] **Step 1: Rewrite the component**

Replace the entire contents of `src/components/SeatEditor.vue` with:

```vue
<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import type { SeatConfig } from "@/lib/types";
import type { PersonaGroup } from "@/lib/personas";
import { isLoopbackUrl, providerIsHttp } from "@/lib/types";
import { NONE, personaById, personasForGroup, groupsForRole } from "@/lib/personas";
import { api } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { ChevronRight } from "@lucide/vue";

const props = defineProps<{ seat: SeatConfig; removable?: boolean }>();
defineEmits<{ remove: [] }>();

const http = computed(() => providerIsHttp(props.seat.provider));
const needsKey = computed(() => http.value && !isLoopbackUrl(props.seat.base_url));
const key = ref("");
const saved = ref<boolean | null>(null);
const showAdvanced = ref(false);

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
async function refresh() { try { saved.value = await api.hasCredential(props.seat.credential_ref); } catch { saved.value = null; } }
async function save() { if (!key.value) return; await api.setCredential(props.seat.credential_ref, key.value); key.value = ""; await refresh(); }
onMounted(refresh);
</script>

<template>
  <article class="terminal-panel p-5">
    <header class="mb-4 flex items-center gap-2 border-b border-line pb-3">
      <span class="rounded border px-1.5 py-0.5 font-mono text-[9px] tracking-wide" :class="seat.role === 'mediator' ? 'border-brass-deep/60 text-brass' : 'border-line-strong text-fg-faint'">{{ seat.role === 'mediator' ? 'MED' : 'SEAT' }}</span>
      <input v-model="seat.display_name" class="min-w-0 flex-1 bg-transparent font-display text-lg text-foreground outline-none" />
      <Button v-if="removable" size="xs" variant="ghost" class="text-deadlock" @click="$emit('remove')">remove</Button>
    </header>

    <div class="grid grid-cols-2 gap-x-4 gap-y-4 font-mono text-[10px] text-fg-muted">
      <label>PROVIDER
        <Select v-model="seat.provider">
          <SelectTrigger class="mt-1 h-9 bg-surface"><SelectValue /></SelectTrigger>
          <SelectContent><SelectGroup>
            <SelectItem value="anthropic">Anthropic</SelectItem>
            <SelectItem value="open_ai_compatible">OpenAI-compatible</SelectItem>
            <SelectItem value="claude_cli">Claude CLI</SelectItem>
            <SelectItem value="codex_cli">Codex CLI</SelectItem>
            <SelectItem value="demo">Demo</SelectItem>
          </SelectGroup></SelectContent>
        </Select>
      </label>
      <label v-if="seat.provider !== 'demo'">MODEL<Input v-model="seat.model" class="mt-1 h-9 bg-surface" /></label>

      <template v-if="seat.role !== 'mediator'">
        <label>TEMPERAMENT
          <Select v-model="temperament">
            <SelectTrigger class="mt-1 h-9 bg-surface"><SelectValue /></SelectTrigger>
            <SelectContent><SelectGroup>
              <SelectItem :value="NONE">— None —</SelectItem>
              <SelectItem v-for="p in temperamentOptions" :key="p.id" :value="p.id">{{ p.label }}</SelectItem>
            </SelectGroup></SelectContent>
          </Select>
        </label>
        <label>DOMAIN EXPERT
          <Select v-model="domain">
            <SelectTrigger class="mt-1 h-9 bg-surface"><SelectValue /></SelectTrigger>
            <SelectContent><SelectGroup>
              <SelectItem :value="NONE">— None —</SelectItem>
              <SelectItem v-for="p in domainOptions" :key="p.id" :value="p.id">{{ p.label }}</SelectItem>
            </SelectGroup></SelectContent>
          </Select>
        </label>
      </template>
      <label v-else class="col-span-2">MEDIATOR PERSONA
        <Select v-model="mediatorPersona">
          <SelectTrigger class="mt-1 h-9 bg-surface"><SelectValue /></SelectTrigger>
          <SelectContent><SelectGroup>
            <SelectItem :value="NONE">— None —</SelectItem>
            <SelectItem v-for="p in mediatorOptions" :key="p.id" :value="p.id">{{ p.label }}</SelectItem>
          </SelectGroup></SelectContent>
        </Select>
      </label>

      <label class="col-span-2">CUSTOM (ADDENDUM)<Textarea v-model="seat.system_prompt" rows="2" placeholder="optional extra instructions, appended after the persona" class="mt-1 resize-y bg-surface text-[11px]" /></label>

      <p v-if="seat.provider === 'demo'" class="col-span-2 text-consensus">offline demo agent, no key required</p>
      <p v-if="seat.provider === 'claude_cli' || seat.provider === 'codex_cli'" class="col-span-2 text-brass">uses local subscription CLI</p>
    </div>

    <div class="mt-4 border-t border-line pt-3">
      <button type="button" class="flex items-center gap-1.5 font-mono text-[10px] uppercase tracking-[0.08em] text-fg-muted transition-colors hover:text-brass" @click="showAdvanced = !showAdvanced">
        <ChevronRight class="size-3 text-brass transition-transform" :class="{ 'rotate-90': showAdvanced }" />
        Advanced
        <span v-if="!showAdvanced" class="ml-1 normal-case tracking-normal text-fg-faint">— base URL, sampling, credentials</span>
      </button>
      <div v-if="showAdvanced" class="mt-3 grid grid-cols-2 gap-x-4 gap-y-4 font-mono text-[10px] text-fg-muted">
        <label v-if="http" class="col-span-2">BASE URL<Input v-model="seat.base_url" class="mt-1 h-9 bg-surface" @blur="refresh" /></label>
        <label>TEMPERATURE<Input v-model.number="seat.sampling.temperature" type="number" step=".05" min="0" max="2" class="mt-1 h-9 bg-surface" /></label>
        <label>TOP P<Input v-model.number="seat.sampling.top_p" type="number" step=".05" min="0" max="1" class="mt-1 h-9 bg-surface" /></label>
        <label>MAX TOKENS<Input v-model.number="seat.sampling.max_tokens" type="number" min="1" class="mt-1 h-9 bg-surface" /></label>
        <template v-if="needsKey">
          <label>CREDENTIAL REF<Input v-model="seat.credential_ref" class="mt-1 h-9 bg-surface" @blur="refresh" /></label>
          <label>API KEY {{ saved ? '[stored]' : '' }}<div class="mt-1 flex gap-1"><Input v-model="key" type="password" class="h-9 bg-surface" /><Button size="sm" variant="outline" @click="save">save</Button></div></label>
        </template>
      </div>
    </div>
  </article>
</template>
```

- [ ] **Step 2: Type check**

Run: `npx vue-tsc --noEmit`
Expected: no errors.

- [ ] **Step 3: Verify in preview**

Start the dev server (`preview_start` → `krunch-dev`), load the setup screen. Confirm:
each seat card now shows provider/model/personas/addendum only; an "Advanced" row sits
below with the hint text; clicking it reveals temperature/top-p/max-tokens (+ base URL
and credentials for Anthropic seats); the chevron rotates. `read_console_messages` →
no errors.

- [ ] **Step 4: Commit**

```bash
git add src/components/SeatEditor.vue
git commit -m "feat(setup): fold sampling/credentials into Advanced disclosure in SeatEditor"
```

---

### Task 3: Roster components + master-detail wiring

Create the roster and rewire the panel section of `SetupScreen.vue` to master-detail:
a `SeatRoster` list on the left, one `SeatEditor` (the selected seat) on the right,
with selection state and the store-mutation watcher. The matter/rules/aside above the
panel section stay as-is this task (replaced in Task 4), so the screen stays functional.

**Files:**
- Create: `src/components/SeatRosterItem.vue`
- Create: `src/components/SeatRoster.vue`
- Modify: `src/screens/SetupScreen.vue` (script + the panel `<section>` only)

**Interfaces:**
- `SeatRosterItem`: props `{ seat: SeatConfig; selected?: boolean }`, emit `select`
- `SeatRoster`: props `{ seats: SeatConfig[]; selectedId: string | null; canAdd: boolean }`, emits `select: [id: string]`, `add`
- `SetupScreen` produces: `selectedSeatId: Ref<string | null>`, `selectedSeat` computed, `selectSeat(id)`, `removeSelected()`

- [ ] **Step 1: Create `SeatRosterItem.vue`**

```vue
<script setup lang="ts">
import { computed } from "vue";
import type { SeatConfig } from "@/lib/types";
import { personaLabels } from "@/lib/personas";

const props = defineProps<{ seat: SeatConfig; selected?: boolean }>();
defineEmits<{ select: [] }>();

const isMediator = computed(() => props.seat.role === "mediator");
const chips = computed(() => personaLabels(props.seat.personas));
const providerLabel = computed(() => ({
  anthropic: "anthropic", open_ai_compatible: "openai", claude_cli: "claude cli", codex_cli: "codex cli", demo: "demo",
}[props.seat.provider]));
const modelLine = computed(() => (props.seat.provider === "demo" ? "demo" : `${providerLabel.value} · ${props.seat.model}`));
</script>

<template>
  <button type="button" :aria-pressed="selected" @click="$emit('select')"
    class="w-full rounded-lg border border-l-2 border-line p-3 text-left transition-colors hover:border-line-strong"
    :class="selected ? 'border-line-strong border-l-brass bg-surface' : 'border-l-transparent bg-bg-deep/60'">
    <div class="flex items-center gap-2">
      <span class="rounded border px-1.5 py-0.5 font-mono text-[9px] tracking-wide" :class="isMediator ? 'border-brass-deep/60 text-brass' : 'border-line-strong text-fg-faint'">{{ isMediator ? 'MED' : 'SEAT' }}</span>
      <span class="min-w-0 flex-1 truncate font-display text-sm text-foreground">{{ seat.display_name }}</span>
    </div>
    <div class="mt-2 flex flex-wrap items-center gap-1.5">
      <span v-for="chip in chips" :key="chip" class="rounded-full bg-brass/15 px-2 py-0.5 font-mono text-[10px] text-brass-bright">{{ chip }}</span>
      <span class="font-mono text-[10px] text-fg-faint">{{ chips.length ? '· ' : '' }}{{ modelLine }}</span>
    </div>
  </button>
</template>
```

- [ ] **Step 2: Create `SeatRoster.vue`**

```vue
<script setup lang="ts">
import type { SeatConfig } from "@/lib/types";
import SeatRosterItem from "./SeatRosterItem.vue";

defineProps<{ seats: SeatConfig[]; selectedId: string | null; canAdd: boolean }>();
defineEmits<{ select: [id: string]; add: [] }>();
</script>

<template>
  <div class="flex flex-col gap-2">
    <SeatRosterItem v-for="seat in seats" :key="seat.id" :seat="seat" :selected="seat.id === selectedId" @select="$emit('select', seat.id)" />
    <button type="button" :disabled="!canAdd" @click="$emit('add')"
      class="rounded-lg border border-dashed border-line p-2.5 text-center font-mono text-[11px] text-fg-faint transition-colors hover:border-brass/50 hover:text-brass disabled:opacity-40 disabled:hover:border-line disabled:hover:text-fg-faint">
      + Add seat <kbd class="opacity-60">A</kbd>
    </button>
  </div>
</template>
```

- [ ] **Step 3: Rewrite `SetupScreen.vue` script + panel section**

Replace the `<script setup>` block of `src/screens/SetupScreen.vue` with:

```vue
<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useIntersectionObserver } from "@vueuse/core";
import { Plus, Play, Sparkles } from "@lucide/vue";
import { useDeliberation } from "@/stores/deliberation";
import SeatRoster from "@/components/SeatRoster.vue";
import SeatEditor from "@/components/SeatEditor.vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";

const store = useDeliberation();
const modeHint = computed(() => ({ autonomous: "no operator pauses", batched: "pause only for unresolved questions", interactive: "pause for each open question" }[store.mode]));
// Display the quorum rounded to 2dp; keep the exact stored fraction (e.g. 2/3)
// so the consensus guard's `cluster_fraction >= quorum_fraction` is unchanged.
const quorumDisplay = computed({
  get: () => Number(store.quorumFraction.toFixed(2)),
  set: (value: number) => { store.quorumFraction = value; },
});

// Seats in bench order: the mediator first, then panelists.
const orderedSeats = computed(() => (store.mediator ? [store.mediator, ...store.panelists] : [...store.panelists]));
const selectedSeatId = ref<string | null>(store.mediator?.id ?? null);
const selectedSeat = computed(() => store.seats.find((seat) => seat.id === selectedSeatId.value) ?? store.mediator ?? null);

// Selection must follow store mutations that bypass the component: the `A`
// shortcut adds a seat straight through the store, and loadDemoPanel replaces
// every seat. Watch the id list — an added id gets selected; if the current
// selection vanished (removed / demo-replaced), fall back to the mediator.
watch(() => store.seats.map((seat) => seat.id), (ids, prev) => {
  const added = ids.find((id) => !prev?.includes(id));
  if (added) { selectedSeatId.value = added; return; }
  if (!selectedSeatId.value || !ids.includes(selectedSeatId.value)) selectedSeatId.value = store.mediator?.id ?? null;
});

function selectSeat(id: string) { selectedSeatId.value = id; }
function removeSelected() {
  const id = selectedSeatId.value;
  if (!id) return;
  selectedSeatId.value = store.mediator?.id ?? null; // reselect before removal so no empty state flashes
  store.removeSeat(id);
}

// Sticky convene bar reveals once the hero convene card scrolls out of view.
const scrollRoot = ref<HTMLElement | null>(null);
const heroCard = ref<HTMLElement | null>(null);
const heroVisible = ref(true);
useIntersectionObserver(heroCard, ([entry]) => { heroVisible.value = entry.isIntersecting; }, { root: scrollRoot });
</script>
```

Then replace the panel `<section>` (the current line beginning
`<section class="terminal-panel p-4"><header ...>The panel // ...`) with:

```vue
        <section class="grid gap-4 lg:grid-cols-[21rem_minmax(0,1fr)]">
          <div>
            <header class="mb-3 flex items-center justify-between">
              <p class="font-mono text-[11px] uppercase tracking-[0.14em] text-brass">The panel // {{ store.panelists.length }}/6 seated</p>
              <Button size="xs" variant="outline" :disabled="store.panelists.length >= 6" class="border-consensus/45 text-consensus" @click="store.addPanelist()"><Plus data-icon="inline-start" />Add seat <kbd class="ml-1 text-fg-faint">A</kbd></Button>
            </header>
            <SeatRoster :seats="orderedSeats" :selected-id="selectedSeatId" :can-add="store.panelists.length < 6" @select="selectSeat" @add="store.addPanelist()" />
          </div>
          <SeatEditor v-if="selectedSeat" :key="selectedSeat.id" :seat="selectedSeat" :removable="selectedSeat.role !== 'mediator'" @remove="removeSelected" />
        </section>
```

(Leave the matter section, the rules section, and the `<aside>` readiness panel
unchanged this task — Task 4 replaces them. The unused `scrollRoot`/`heroCard`/
`heroVisible` refs are wired to the DOM in Task 5; they type-check fine unused.)

- [ ] **Step 4: Type check**

Run: `npx vue-tsc --noEmit`
Expected: no errors.

- [ ] **Step 5: Verify in preview**

Reload the setup screen. Confirm: the panel section is now a roster (left) + one
editor (right); the mediator is selected by default; clicking a roster row swaps the
editor and highlights the row (brass left edge); "+ Add seat" (button, ghost row, and
the `A` key) adds a panelist and selects it; removing the selected panelist falls back
to the mediator; "Load demo panel" repopulates and selects the demo mediator.
`read_console_messages` → no errors.

- [ ] **Step 6: Commit**

```bash
git add src/components/SeatRosterItem.vue src/components/SeatRoster.vue src/screens/SetupScreen.vue
git commit -m "feat(setup): master-detail seat roster + detail editor"
```

---

### Task 4: `ConvenePanel` + hero and rules layout

Extract readiness + Convene into one `ConvenePanel` (variant `card` | `bar`), place
the `card` beside the matter as the hero, and demote the rules to a thin strip.
Removes the old floating `<aside>`.

**Files:**
- Create: `src/components/ConvenePanel.vue`
- Modify: `src/screens/SetupScreen.vue` (template: hero grid, rules strip; drop old aside + the old outer grid wrapper)

**Interfaces:**
- `ConvenePanel`: props `{ variant: "card" | "bar" }`; reads the store; calls `store.start()`

- [ ] **Step 1: Create `ConvenePanel.vue`**

```vue
<script setup lang="ts">
import { computed } from "vue";
import { Play } from "@lucide/vue";
import { useDeliberation } from "@/stores/deliberation";
import { Button } from "@/components/ui/button";

defineProps<{ variant: "card" | "bar" }>();
const store = useDeliberation();
const ready = computed(() => store.validation.length === 0);
const seatWord = computed(() => (store.panelists.length === 1 ? "seat" : "seats"));
const summary = computed(() => `1 med · ${store.panelists.length} ${seatWord.value} · ${store.mode} · max ${store.maxRounds}`);
</script>

<template>
  <div v-if="variant === 'card'" class="terminal-panel flex flex-col p-4">
    <p class="font-mono text-[11px] uppercase tracking-[0.14em] text-brass">Readiness</p>
    <dl class="mt-3 grid grid-cols-[auto_1fr] gap-x-3 gap-y-1.5 font-mono text-[10px]">
      <dt class="text-fg-faint">panel</dt><dd class="text-right text-fg-muted">1 med · {{ store.panelists.length }} {{ seatWord }}</dd>
      <dt class="text-fg-faint">mode</dt><dd class="text-right text-fg-muted">{{ store.mode }}</dd>
      <dt class="text-fg-faint">rounds</dt><dd class="text-right text-fg-muted">max {{ store.maxRounds }}</dd>
    </dl>
    <ul class="mt-4 space-y-2 border-t border-line pt-4 font-mono text-[10px]">
      <li v-if="ready" class="text-consensus">[✓] ready to convene</li>
      <li v-for="item in store.validation" :key="item" class="text-deadlock">[!] {{ item }}</li>
    </ul>
    <p v-if="store.startError" class="mt-3 border-t border-deadlock/40 pt-3 text-[10px] text-deadlock">{{ store.startError }}</p>
    <Button class="mt-5 w-full bg-consensus font-mono text-primary-foreground hover:bg-consensus/85" :disabled="!ready" @click="store.start()"><Play data-icon="inline-start" />Convene panel <kbd class="ml-1 opacity-70">C</kbd></Button>
  </div>

  <div v-else class="flex items-center gap-4 border-t border-line-strong bg-surface/95 px-5 py-3">
    <span class="font-mono text-[11px] text-fg-muted">{{ summary }}</span>
    <span v-if="ready" class="font-mono text-[11px] text-consensus">[✓] ready</span>
    <span v-else class="font-mono text-[11px] text-deadlock">[!] {{ store.validation.length }} to resolve</span>
    <div class="flex-1"></div>
    <Button size="sm" class="bg-consensus font-mono text-primary-foreground hover:bg-consensus/85" :disabled="!ready" @click="store.start()"><Play data-icon="inline-start" />Convene panel <kbd class="ml-1 opacity-70">C</kbd></Button>
  </div>
</template>
```

- [ ] **Step 2: Wire the hero + rules into `SetupScreen.vue`**

Add the import to the `<script setup>` (below the `SeatEditor` import):

```ts
import ConvenePanel from "@/components/ConvenePanel.vue";
```

Replace the `<template>` block with this full version (hero grid + thin rules strip +
Task-3 panel section; the sticky bar element is added in Task 5):

```vue
<template>
  <main ref="scrollRoot" class="relative min-h-0 flex-1 overflow-y-auto">
    <div class="mx-auto max-w-[92rem] space-y-5 p-4 lg:p-6">
      <header class="flex flex-wrap items-end justify-between gap-4 border-b border-line pb-4">
        <div>
          <h1 class="font-display text-3xl text-foreground">Convene the panel</h1>
          <p class="mt-2 text-sm text-fg-muted">State the matter, seat the panel, then open deliberation.</p>
        </div>
        <Button size="sm" variant="outline" class="border-brass/50 text-brass" @click="store.loadDemoPanel()"><Sparkles data-icon="inline-start" />Load demo panel</Button>
      </header>

      <div class="grid gap-5 lg:grid-cols-[minmax(0,1fr)_20rem]">
        <section class="terminal-panel p-5">
          <p class="mb-2.5 font-mono text-[11px] uppercase tracking-[0.14em] text-brass">The matter</p>
          <Textarea v-model="store.problem" rows="6" placeholder="State the matter to deliberate…" class="resize-none bg-bg-deep text-sm leading-relaxed" />
        </section>
        <div ref="heroCard"><ConvenePanel variant="card" /></div>
      </div>

      <section class="terminal-panel flex flex-wrap items-center gap-x-8 gap-y-4 p-4">
        <label class="font-mono text-[10px] text-fg-muted">INTERACTION MODE
          <ToggleGroup v-model="store.mode" type="single" variant="outline" class="mt-1.5 grid grid-cols-3">
            <ToggleGroupItem value="autonomous">AUTO</ToggleGroupItem>
            <ToggleGroupItem value="batched">BATCH</ToggleGroupItem>
            <ToggleGroupItem value="interactive">LIVE</ToggleGroupItem>
          </ToggleGroup>
        </label>
        <label class="font-mono text-[10px] text-fg-muted">MAX ROUNDS<Input v-model.number="store.maxRounds" type="number" min="1" max="64" class="mt-1.5 w-24 bg-bg-deep" /></label>
        <label class="font-mono text-[10px] text-fg-muted">QUORUM<Input v-model.number="quorumDisplay" type="number" min="0" max="1" step=".05" class="mt-1.5 w-24 bg-bg-deep" /></label>
        <label class="font-mono text-[10px] text-fg-muted">CONFIDENCE<Input v-model.number="store.confidenceFloor" type="number" min="0" max="1" step=".05" class="mt-1.5 w-24 bg-bg-deep" /></label>
        <span class="ml-auto font-mono text-[10px] text-fg-faint">{{ modeHint }}</span>
      </section>

      <section class="grid gap-4 lg:grid-cols-[21rem_minmax(0,1fr)]">
        <div>
          <header class="mb-3 flex items-center justify-between">
            <p class="font-mono text-[11px] uppercase tracking-[0.14em] text-brass">The panel // {{ store.panelists.length }}/6 seated</p>
            <Button size="xs" variant="outline" :disabled="store.panelists.length >= 6" class="border-consensus/45 text-consensus" @click="store.addPanelist()"><Plus data-icon="inline-start" />Add seat <kbd class="ml-1 text-fg-faint">A</kbd></Button>
          </header>
          <SeatRoster :seats="orderedSeats" :selected-id="selectedSeatId" :can-add="store.panelists.length < 6" @select="selectSeat" @add="store.addPanelist()" />
        </div>
        <SeatEditor v-if="selectedSeat" :key="selectedSeat.id" :seat="selectedSeat" :removable="selectedSeat.role !== 'mediator'" @remove="removeSelected" />
      </section>
    </div>
  </main>
</template>
```

- [ ] **Step 3: Type check**

Run: `npx vue-tsc --noEmit`
Expected: no errors.

- [ ] **Step 4: Verify in preview**

Reload. Confirm: matter textarea (hero, left) sits beside the readiness/Convene card
(right); the rules are a single thin strip with the mode hint pushed right; the old
floating sidebar is gone; Convene is disabled with an empty matter and enables once
matter + panel are valid; clicking Convene (or pressing `C`) starts a session
(navigates to the room, or logs a start error in preview without Tauri).
`read_console_messages` → no errors. Screenshot as proof.

- [ ] **Step 5: Commit**

```bash
git add src/components/ConvenePanel.vue src/screens/SetupScreen.vue
git commit -m "feat(setup): hero matter + ConvenePanel card, thin rules strip"
```

---

### Task 5: Sticky convene bar

Render `ConvenePanel variant="bar"` pinned to the bottom of the scroll container,
revealed once the hero card scrolls out of view (the IntersectionObserver from Task 3
is already wired to `heroCard`/`scrollRoot`).

**Files:**
- Modify: `src/screens/SetupScreen.vue` (template only — add the sticky element)

- [ ] **Step 1: Add the sticky bar**

In `src/screens/SetupScreen.vue`, add this as the last child of `<main>` (immediately
before `</main>`, after the `</div>` that closes the `max-w` container):

```vue
    <Transition name="fade">
      <div v-show="!heroVisible" class="sticky bottom-0 z-20"><ConvenePanel variant="bar" /></div>
    </Transition>
```

- [ ] **Step 2: Type check**

Run: `npx vue-tsc --noEmit`
Expected: no errors.

- [ ] **Step 3: Verify in preview**

Reload with the demo panel loaded (so the page is tall enough to scroll). Confirm: the
sticky bar is hidden at the top; scrolling down until the hero card leaves view fades
the bar in at the bottom with the readiness summary and a working Convene button;
scrolling back up hides it. Test with `resize_window` to a shorter height if needed to
force a scroll. `read_console_messages` → no errors.

- [ ] **Step 4: Commit**

```bash
git add src/screens/SetupScreen.vue
git commit -m "feat(setup): sticky convene bar revealed on scroll"
```

---

### Task 6: Polish, responsive, and full verification

Final pass: reduced-motion, single-column collapse below `lg`, and an
end-to-end verification with proof.

**Files:**
- Modify: `src/screens/SetupScreen.vue` (only if the checks below surface issues)

- [ ] **Step 1: Confirm reduced-motion**

The chevron rotation and the `.fade` transition are the only added motion. `.fade` is
already covered by the global `prefers-reduced-motion` rule in `src/style.css`
(line ~157). Verify with `resize_window` `colorScheme`/DevTools emulation of
reduced-motion that the bar still appears (just without the fade). No code change
expected; if the chevron animates under reduced-motion, add
`motion-reduce:transition-none` to its class.

- [ ] **Step 2: Confirm single-column collapse**

`resize_window` to width < 1000px. Confirm the hero grid and the panel grid both
collapse to a single column (roster above editor) with no horizontal overflow —
`minmax(0,1fr)` and the `lg:` prefixes handle this. If any element overflows, add
`min-w-0` to the offending grid child. Restore to desktop width after.

- [ ] **Step 3: Full type check + build**

Run: `npm run build`
Expected: `vue-tsc --noEmit` passes and `vite build` succeeds.

- [ ] **Step 4: End-to-end verification with proof**

Fresh reload. Walk the full flow: type a matter → load demo panel → click through each
roster seat (editor swaps) → toggle an Advanced section → change mode/rounds (readiness
card + sticky summary update) → add a seat to 6 (add controls disable) → remove back
down → press `C` to convene. `read_console_messages` clean throughout. Capture a
`screenshot` of the finished screen as proof.

- [ ] **Step 5: Commit (if Step 1/2 required changes)**

```bash
git add src/screens/SetupScreen.vue
git commit -m "fix(setup): reduced-motion + single-column polish"
```

---

## Self-Review

**Spec coverage:**
- Density / repetition → Task 2 (Advanced disclosure) + Task 3 (one editor, not N). ✓
- Balance / whitespace → Task 3 (roster list, no stranded grid) + Task 4 (no floating aside). ✓
- Weak CTA → Task 4 (hero ConvenePanel card) + Task 5 (sticky bar). ✓
- Flow → Task 4 (matter hero → rules strip → panel order). ✓
- `SetupScreen` orchestrator, `SeatRoster`+`SeatRosterItem`, `SeatEditor` detail,
  single `ConvenePanel` two placements → Tasks 2–5. ✓
- Default selection = mediator; add selects new; remove falls back to mediator;
  persona chips derived; keep `A`/`C`; responsive single column → Tasks 3, 6. ✓
- Aesthetic unchanged (warm chamber tokens) → Global Constraints; all classes use
  existing tokens. ✓

**Deviation from spec (noted):** The spec floated an optional roster "credential
missing" affordance. Dropped to keep `SeatRosterItem` purely prop-driven (no per-row
async `api.hasCredential`). This hides nothing new: `store.validation` already flags an
empty `credential_ref`, and the stored-key check was never part of validation
(pre-existing, out of scope). Not a spec requirement — the spec called it optional.

**Placeholder scan:** No TBD/TODO; every code step contains complete file contents or
exact insertions. ✓

**Type consistency:** `personaLabels(ids)` (Task 1) consumed in `SeatRosterItem`
(Task 3). `ConvenePanel` prop `variant: "card" | "bar"` used consistently (Tasks 4–5).
`selectedSeatId`/`selectedSeat`/`selectSeat`/`removeSelected` defined in Task 3 script,
consumed in Tasks 3–5 templates. `scrollRoot`/`heroCard`/`heroVisible` defined Task 3,
bound to DOM Tasks 4–5. `SeatEditor` prop/emit signature unchanged across tasks. ✓

**Carrying the pattern to the Room** is documentation-only in the spec — no task, by
design (out of scope). ✓
