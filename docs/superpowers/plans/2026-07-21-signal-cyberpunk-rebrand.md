# krunch "Signal" Cyberpunk Rebrand — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Craft note:** this is a visual rebrand. For any task that produces UI, the implementer should invoke the `impeccable:impeccable` (or `frontend-design`) skill for the craft pass, and MUST verify visually in the running app (see each task's verification). The code blocks below are correct, working baselines — not sacred; polish them against the screenshots.

**Goal:** Replace krunch's warm "deliberation chamber" look with a sci-fi/cyberpunk **ops-terminal** identity — neon green/red on pure black, Monaspace-only type, phosphor glow + moving grid + HUD chrome effects, and a new oscilloscope signature moment where panelist signals phase-lock as the panel converges.

**Architecture:** Surface-only reskin. Almost all work is in [src/style.css](../../../src/style.css) (design tokens + effects), a mechanical `brass → signal` rename across components, one new canvas component (`OscilloscopeSync.vue`) driven by existing store data, and small per-component class tweaks. No backend, wire-type, or store-logic changes.

**Tech Stack:** Vue 3 (`<script setup>`), Tailwind CSS v4 (`@theme inline` + CSS custom properties, oklch colors), shadcn-vue / reka-ui, Tauri 2 (untouched), Vite (dev server `krunch-frontend` on port 1420).

## Global Constraints

- **No backend / data-model / store-logic changes.** `crates/`, `src-tauri/`, [src/lib/types.ts](../../../src/lib/types.ts), and all logic in [src/stores/deliberation.ts](../../../src/stores/deliberation.ts) are off-limits. Reskin only.
- **Monaspace family only — no new font dependency.** Use faces already present in [src/assets/fonts](../../../src/assets/fonts): Departure Mono (pixel), Monaspace Neon/Argon/Krypton/Xenon. Young Serif + Hanken stay in the repo but drop out of active use.
- **All effects wired to the existing controls:** the `--effects-intensity` CSS var, the `off / ambient / max` toggle, the `prefers-reduced-motion` guard, and the rAF perf auto-reduce loop in [src/App.vue](../../../src/App.vue). New animation must go static when `store.instantTokens === true`.
- **Keep the token architecture** (same CSS variable names) so component churn stays minimal; the one rename is `brass → signal`.
- **Pure-black base, surfaces slightly lifted** so panels read against the void and neon doesn't fatigue.
- **Departure Mono is pixel type — large sizes only**, never body text.
- **Palette values are approximate** (oklch) — tune against screenshots during implementation.

---

## File Structure

- `src/assets/fonts/fonts.css` — **modify**: declare Departure Mono + Monaspace Neon/Argon/Krypton `@font-face`.
- `src/style.css` — **modify** (bulk of the work): token values, `@theme` font/color vars, effects layers, panel chrome, markdown prose.
- `src/App.vue` — **modify**: background layer swap, remove glow machinery, boot handshake overlay.
- `src/components/OscilloscopeSync.vue` — **create**: canvas signature moment.
- `src/screens/RoomScreen.vue` — **modify**: mount the oscilloscope band.
- `src/components/CockpitStatusBar.vue` — **modify**: remove `ConvergenceStrip`, HUD ticker/labels.
- `src/components/ConvergenceStrip.vue` — **modify**: becomes the oscilloscope's readout row.
- `src/components/SeatCard.vue`, `MediatorPanel.vue`, `EventLogRail.vue`, `src/screens/SetupScreen.vue`, `src/screens/VerdictScreen.vue`, and the rest under `src/components/**` — **modify**: `brass → signal` rename + targeted class polish.
- `.impeccable.md` — **modify**: full rewrite of the design-system doc.

---

## Task 1: Declare the Monaspace + Departure font faces

Only Young Serif, Hanken, Monaspace Xenon, and Fragment Mono are declared today. The faces the rebrand needs (Departure Mono, Monaspace Neon/Argon/Krypton) exist as files but have no `@font-face`, so `font-family` references would silently fall back. Declare them.

**Files:**
- Modify: `src/assets/fonts/fonts.css`

**Interfaces:**
- Produces: usable `font-family` names `"Departure Mono"`, `"Monaspace Neon"`, `"Monaspace Argon"`, `"Monaspace Krypton"` (Task 3 wires them into `@theme`).

- [ ] **Step 1: Append the new `@font-face` blocks**

Append to `src/assets/fonts/fonts.css` (the `.woff2` paths match the existing folders under `src/assets/fonts/`):

```css
/* ── Cyberpunk "Signal" rebrand faces ─────────────────────────────────────
   Departure Mono (pixel): large headings, wordmark, verdict title.
   Monaspace Neon (neo-grotesque): UI / chrome / buttons.
   Monaspace Argon (humanist): streamed deliberation prose (legible long-form).
   Monaspace Krypton (mechanical): telemetry — seat ids, %, counters, models. */

@font-face {
  font-family: 'Departure Mono';
  font-style: normal;
  font-weight: 400;
  font-display: swap;
  src: url('./departure-mono-regular.woff2') format('woff2');
}
@font-face {
  font-family: 'Monaspace Neon';
  font-style: normal;
  font-weight: 200 800;
  font-display: swap;
  src: url('./Monaspace Neon/Monaspace Neon Var.woff2') format('woff2');
}
@font-face {
  font-family: 'Monaspace Argon';
  font-style: normal;
  font-weight: 200 800;
  font-display: swap;
  src: url('./Monaspace Argon/Monaspace Argon Var.woff2') format('woff2');
}
@font-face {
  font-family: 'Monaspace Krypton';
  font-style: normal;
  font-weight: 200 800;
  font-display: swap;
  src: url('./Monaspace Krypton/Monaspace Krypton Var.woff2') format('woff2');
}
```

- [ ] **Step 2: Start the dev server and verify fonts load without 404**

Use the preview tool: `preview_start` with `{name: "krunch-frontend"}` (port 1420). Then `read_network_requests` with `urlPattern: "woff2"`.
Expected: requests for `departure-mono-regular.woff2`, `Monaspace Neon/Argon/Krypton Var.woff2` all return 200 (they load lazily when first used — if none appear yet, that's fine; Task 3 will exercise them). No 404s.

- [ ] **Step 3: Commit**

```bash
git add src/assets/fonts/fonts.css
git commit -m "feat(fonts): declare Departure Mono + Monaspace Neon/Argon/Krypton faces"
```

---

## Task 2: Mechanical `brass → signal` rename (no visual change)

`brass` is semantically wrong for a green terminal. Rename it everywhere **before** changing values, as a pure find-and-replace: token names, `@theme` color entries, and every `text-brass`/`bg-brass`/`border-brass`/`ring-brass`/`brass/NN` utility across components. The app stays visually identical (still warm) — this task is a rename only, which makes it trivial to review.

**Files:**
- Modify: `src/style.css` (26 occurrences) and every component under `src/components/**` + `src/screens/**` that uses `brass` (60 occurrences across 15 files).

**Interfaces:**
- Produces: tokens `--signal`, `--signal-bright`, `--signal-deep`; `@theme` vars `--color-signal*`; utility classes `text-signal`, `bg-signal`, `border-signal`, `ring-signal`, and opacity variants `signal/NN`.

- [ ] **Step 1: Rename across the whole `src` tree**

The identifier `brass` only ever refers to this token (verified: no unrelated "brass" strings). A global word-boundary replace is safe:

```bash
cd /Users/la.kyle.dougan/git/personal/krunch
grep -rl 'brass' src | xargs sed -i '' -E 's/brass/signal/g'
```

This turns `--brass` → `--signal`, `--brass-bright` → `--signal-bright`, `--brass-deep` → `--signal-deep`, `--color-brass*` → `--color-signal*`, and all `*-brass` / `brass/NN` utility classes → `*-signal` / `signal/NN`.

- [ ] **Step 2: Verify no `brass` remains**

```bash
grep -rn 'brass' src ; echo "exit: $?"
```
Expected: no matches (grep exit 1). If any remain, fix them.

- [ ] **Step 3: Typecheck**

```bash
npx vue-tsc --noEmit
```
Expected: passes (rename doesn't affect types; this confirms no `.vue` template got mangled).

- [ ] **Step 4: Verify the app still renders (unchanged, still warm)**

`preview_start` `{name: "krunch-frontend"}`, then `navigate` to `http://localhost:1420/?preview=stream`. `read_console_messages` `{onlyErrors: true}`.
Expected: no errors; app looks identical to before (warm brass palette) — this task changed names, not values.

- [ ] **Step 5: Commit**

```bash
git add -A src
git commit -m "refactor(theme): rename brass token -> signal (no visual change)"
```

---

## Task 3: Remap palette + font tokens → green/red terminal + Monaspace

Flip the look in one edit: remap every color token value to the green/red terminal palette and repoint the font vars to Monaspace. Because Task 2 already renamed `brass → signal`, this is value-only. After this task the whole app reads green/red and mono.

**Files:**
- Modify: `src/style.css` (the `:root` block lines ~6-56 and the `@theme inline` font lines ~64-66)

**Interfaces:**
- Consumes: `--signal*` names from Task 2; font-family names from Task 1.
- Produces: the final palette + typographic tokens all other tasks build on.

- [ ] **Step 1: Replace the `:root` token block**

Replace the entire `:root { … }` block in `src/style.css` (the warm-chamber tokens, currently lines ~6-56) with:

```css
:root {
  /* ── Ops terminal ─────────────────────────────────────────────────────
     A signal room at night. Pure-black void; surfaces lifted just enough to
     read. Neutrals carry a faint green cast (hue ~165). Ink is phosphor. */
  --bg: oklch(0.150 0.012 165);
  --bg-deep: oklch(0 0 0);
  --surface: oklch(0.190 0.014 165);
  --surface-2: oklch(0.235 0.016 165);
  --surface-3: oklch(0.285 0.018 165);
  --fg: oklch(0.940 0.020 165);
  --fg-muted: oklch(0.750 0.030 165);
  --fg-faint: oklch(0.570 0.030 165);
  --line: oklch(0.320 0.020 165);
  --line-strong: oklch(0.480 0.040 165);

  /* Neon green — the single primary. Signal, lock, the live trace. */
  --signal: oklch(0.850 0.210 160);
  --signal-bright: oklch(0.900 0.230 160);
  --signal-deep: oklch(0.620 0.160 160);

  /* Deliberation states: green as the panel locks in, dim while it runs,
     neon red at deadlock / alert. */
  --consensus: oklch(0.880 0.240 158);
  --continue: var(--signal);
  --deadlock: oklch(0.620 0.250 22);
  --danger: oklch(0.620 0.250 22);

  --radius: 0.375rem;
  --effects-intensity: 0.55;

  /* shadcn / reka-ui bindings */
  --background: var(--bg);
  --foreground: var(--fg);
  --card: var(--surface);
  --card-foreground: var(--fg);
  --popover: var(--surface-2);
  --popover-foreground: var(--fg);
  --primary: var(--signal);
  --primary-foreground: oklch(0.140 0.020 165);
  --secondary: var(--surface-2);
  --secondary-foreground: var(--fg);
  --muted: var(--surface-2);
  --muted-foreground: var(--fg-muted);
  --accent: var(--surface-3);
  --accent-foreground: var(--fg);
  --destructive: var(--deadlock);
  --destructive-foreground: oklch(0.980 0.010 160);
  --border: var(--line);
  --input: var(--surface-2);
  --ring: var(--signal);
}
```

- [ ] **Step 2: Repoint the font vars in `@theme inline`**

In the `@theme inline { … }` block, replace the three font lines (currently `--font-display`, `--font-sans`, `--font-mono`) with:

```css
  --font-display: "Departure Mono", ui-monospace, monospace;
  --font-sans: "Monaspace Neon", ui-monospace, "SF Mono", monospace;
  --font-mono: "Monaspace Krypton", "Monaspace Xenon", ui-monospace, "SF Mono", monospace;
  --font-prose: "Monaspace Argon", ui-sans-serif, system-ui, sans-serif;
  --font-record: "Monaspace Xenon", ui-serif, Georgia, serif;
```

`--font-prose` and `--font-record` are consumed as raw CSS vars (`var(--font-prose)`) in Tasks 5-6, so they only need to *exist* as custom properties. Keeping them inside `@theme inline` is sufficient — Tailwind v4 also auto-exposes them as `font-prose` / `font-record` utilities, but we don't rely on that.

- [ ] **Step 3: Update the `.font-display` helper**

Replace the `.font-display` rule (currently applies Young Serif tracking) with a pixel-appropriate one:

```css
.font-display { font-family: var(--font-display); font-weight: 400; letter-spacing: 0.01em; text-transform: uppercase; }
```

Set `body { … font-family: var(--font-sans); … }` — it already uses `var(--font-sans)`, so no change needed there beyond confirming.

- [ ] **Step 4: Typecheck + visual check across screens**

```bash
npx vue-tsc --noEmit
```
Then `preview_start` `{name: "krunch-frontend"}` and screenshot each: `/?preview=stream`, `/?preview=verdict`, `/?preview=deadlock`, and `/` (setup). Use `computer {action: "screenshot"}`.
Expected: everything is now green/red on near-black, type is Monaspace. It will look raw (old warm-panel gradient still present, no oscilloscope yet) — that's expected; later tasks refine.

- [ ] **Step 5: Commit**

```bash
git add src/style.css
git commit -m "feat(theme): remap palette to green/red terminal + Monaspace type"
```

---

## Task 4: Background + panel effects — grid/dataflow, phosphor glow, HUD chrome, glitch

Retire the warm atmosphere (`.chamber-glow` radial, warm vignette, grain) and the glow machinery in App.vue. Add the cyberpunk effect layers, all gated on `--effects-intensity` (which is `0` when effects are off/reduced).

**Files:**
- Modify: `src/style.css` (the `@property` glow vars ~90-91, `.chamber-glow` ~92-96, `.chamber::after` vignette ~98, `body::after` grain ~100, and `.terminal-panel` ~78-84)
- Modify: `src/App.vue` (remove `glow`/`glowStyle` computed + the `.chamber-glow` div; add the grid layer)

**Interfaces:**
- Consumes: `--effects-intensity`, `--signal`, `--deadlock`, `--line` tokens.
- Produces: `.hud-grid` background layer; reskinned `.terminal-panel`; `.glitch` utility + `@keyframes glitch`; phosphor `.glow-text` / `.glow-edge` helpers.

- [ ] **Step 1: Replace `.terminal-panel` with a notched HUD panel**

Replace the `.terminal-panel { … }` rule with:

```css
/* HUD panel: sharp graphite surface, hairline border, a phosphor top edge, and
   clipped/notched top-right corner. Corner bracket drawn with the ::before. */
.terminal-panel {
  position: relative;
  background: var(--surface);
  border: 1px solid var(--line);
  border-radius: var(--radius);
  box-shadow:
    inset 0 0 0 1px color-mix(in oklch, var(--signal) calc(6% * var(--effects-intensity)), transparent),
    inset 0 1px 0 color-mix(in oklch, var(--signal) 10%, transparent),
    0 0 calc(28px * var(--effects-intensity)) -14px color-mix(in oklch, var(--signal) 60%, transparent);
  clip-path: polygon(0 0, calc(100% - 10px) 0, 100% 10px, 100% 100%, 0 100%);
  transition: border-color 240ms ease, box-shadow 240ms ease;
}
.terminal-panel::before {
  content: ""; position: absolute; top: 6px; left: 6px; width: 10px; height: 10px;
  border-top: 1px solid var(--signal); border-left: 1px solid var(--signal);
  opacity: calc(0.5 * var(--effects-intensity)); pointer-events: none;
}
```

- [ ] **Step 2: Replace the glow `@property` + `.chamber-glow` + vignette + grain rules**

Delete the two `@property --glow-*` declarations, the `.chamber-glow` rule, the `.chamber::after` vignette rule, and the `body::after` grain rule. Replace with the moving grid + dataflow + phosphor helpers + glitch:

```css
/* Moving dot/line grid behind everything — depth + "the system is running".
   Transform-only animation (GPU-cheap). Fades out with --effects-intensity. */
.hud-grid {
  position: fixed; inset: -40px; z-index: 0; pointer-events: none;
  opacity: calc(0.5 * var(--effects-intensity));
  background-image:
    linear-gradient(to right, color-mix(in oklch, var(--signal) 8%, transparent) 1px, transparent 1px),
    linear-gradient(to bottom, color-mix(in oklch, var(--signal) 8%, transparent) 1px, transparent 1px);
  background-size: 44px 44px;
  animation: grid-drift 18s linear infinite;
  mask-image: radial-gradient(120% 120% at 50% 40%, #000 30%, transparent 90%);
}
.hud-grid::after {
  content: ""; position: absolute; inset: 0;
  background: linear-gradient(115deg, transparent 40%, color-mix(in oklch, var(--signal) 22%, transparent) 50%, transparent 60%);
  background-size: 300% 300%;
  animation: dataflow 9s ease-in-out infinite;
  opacity: calc(0.4 * var(--effects-intensity));
}
@keyframes grid-drift { to { transform: translate(44px, 44px); } }
@keyframes dataflow { 0%,100% { background-position: 0% 0%; } 50% { background-position: 100% 100%; } }

/* Phosphor glow helpers — apply where a neon element should bloom. */
.glow-text { text-shadow: 0 0 calc(8px * var(--effects-intensity)) color-mix(in oklch, currentColor 70%, transparent); }
.glow-edge { box-shadow: 0 0 calc(16px * var(--effects-intensity)) -6px color-mix(in oklch, var(--signal) 80%, transparent); }

/* Glitch: brief RGB-split shudder on state changes / errors. */
@keyframes glitch {
  0%,100% { transform: none; clip-path: none; }
  20% { transform: translateX(-2px); }
  40% { transform: translateX(2px); text-shadow: 1.5px 0 var(--deadlock), -1.5px 0 var(--signal); }
  60% { transform: translateX(-1px); text-shadow: -1.5px 0 var(--deadlock), 1.5px 0 var(--signal); }
  80% { transform: translateX(1px); }
}
.glitch { animation: glitch 320ms steps(3, end); }
```

- [ ] **Step 3: Swap the background layer + remove glow machinery in `src/App.vue`**

In the template, replace `<div class="chamber-glow" :style="glowStyle" />` with:

```html
    <div class="hud-grid" />
```

In `<script setup>`, delete the `glow` computed and the `glowStyle` computed (lines ~38-46). They are now unused — the oscilloscope (Task 6) reads `store.convergence` directly.

- [ ] **Step 4: Typecheck + verify effects toggle**

```bash
npx vue-tsc --noEmit
```
Expected: passes (no dangling refs to `glow`/`glowStyle`).
`preview_start`, navigate `/?preview=stream`, screenshot. Then set effects off: `javascript_tool` → `document.querySelector('.chamber').style.setProperty('--effects-intensity','0')`, screenshot again.
Expected: with intensity 0.55 the grid drifts + panels glow faintly; at 0 the grid and glow vanish (static), panels flat. Confirm the ABORT/round HUD still reads.

- [ ] **Step 5: Commit**

```bash
git add src/style.css src/App.vue
git commit -m "feat(effects): grid/dataflow bg, phosphor panels, glitch; drop warm glow"
```

---

## Task 5: Reskin streamed-prose markdown

The deliberation transcript is long-form; set it in the humanist Argon face with green accents and Xenon code, replacing the warm serif headings.

**Files:**
- Modify: `src/style.css` (the `.markdown-body …` block, currently lines ~132-154)

**Interfaces:**
- Consumes: `--font-prose`, `--font-record`, `--signal*`, `--fg*` tokens.

- [ ] **Step 1: Rewrite the `.markdown-body` rules**

Replace the `.markdown-body` block. Keep the inline-display wrappers (`.markdown-body`, `.markdown-body > div`, block-margins) exactly as-is; change the typographic rules:

```css
.markdown-body { display: inline; font-family: var(--font-prose); }
.markdown-body > div { display: inline; }
.markdown-body :where(p, ul, ol, pre, blockquote, table, h1, h2, h3, h4, h5, h6) { display: block; margin: 0 0 0.72em; }
.markdown-body :where(p, ul, ol, pre, blockquote, table, h1, h2, h3, h4, h5, h6):last-child { margin-bottom: 0; }
.markdown-body :where(h1, h2, h3, h4, h5, h6) { font-family: var(--font-mono); color: var(--signal-bright); font-weight: 600; letter-spacing: 0.02em; text-transform: uppercase; line-height: 1.3; }
.markdown-body h1 { font-size: 1.2em; } .markdown-body h2 { font-size: 1.1em; } .markdown-body h3 { font-size: 1em; }
.markdown-body :where(strong, b) { color: var(--signal-bright); font-weight: 600; }
.markdown-body :where(em, i) { color: var(--fg-muted); font-style: italic; }
.markdown-body a { color: var(--signal); text-decoration: underline; text-underline-offset: 2px; text-decoration-color: color-mix(in oklch, var(--signal) 55%, transparent); }
.markdown-body :where(ul, ol) { padding-left: 1.4em; }
.markdown-body ul { list-style: none; }
.markdown-body ul > li::before { content: "\203A"; color: var(--signal); margin-left: -1em; margin-right: 0.5em; }
.markdown-body ol { list-style: decimal; }
.markdown-body li { margin: 0.18em 0; }
.markdown-body :where(code) { font-family: var(--font-record); font-size: 0.92em; background: color-mix(in oklch, var(--signal) 12%, transparent); border-radius: 4px; padding: 0.05em 0.32em; }
.markdown-body pre { background: var(--bg-deep); border: 1px solid var(--line); border-radius: var(--radius); padding: 0.72em 0.9em; overflow-x: auto; }
.markdown-body pre code { background: none; padding: 0; }
.markdown-body blockquote { border-left: 2px solid var(--signal); padding-left: 0.85em; color: var(--fg-muted); font-style: italic; }
.markdown-body hr { border: none; border-top: 1px dashed var(--line-strong); margin: 0.85em 0; }
.markdown-body :where(th, td) { border: 1px solid var(--line); padding: 0.28em 0.6em; text-align: left; }
.markdown-body th { color: var(--signal); font-weight: 600; }
```

- [ ] **Step 2: Verify prose rendering**

`preview_start`, navigate `/?preview=stream`, wait for streaming (`computer {action: "wait", duration: 6}`), screenshot a seat card.
Expected: transcript prose reads in Argon (humanist mono), headings uppercase green, code in Xenon slab, `›` green bullets. Readable, not eye-searing.

- [ ] **Step 3: Commit**

```bash
git add src/style.css
git commit -m "feat(theme): reskin streamed-prose markdown to Argon + green accents"
```

---

## Task 6: OscilloscopeSync — the signature moment

A canvas band at the top of the Room screen. Each panelist is a waveform channel (amplitude ∝ its `confidence`); channels sharing the majority stance phase-align; `clusterFraction`/`meanConfidence`/`effectiveRuling` drive how tightly they fold into one clean green trace (CONSENSUS) vs. desync into red noise (DEADLOCK). Goes static when `store.instantTokens` is true (effects off / reduced-motion / perf-degraded), which hooks the existing auto-reduce loop for free.

**Files:**
- Create: `src/components/OscilloscopeSync.vue`
- Modify: `src/screens/RoomScreen.vue` (mount the band)
- Modify: `src/components/ConvergenceStrip.vue` (becomes the readout row — layout tweak only)
- Modify: `src/components/CockpitStatusBar.vue` (remove the now-relocated `ConvergenceStrip`)

**Interfaces:**
- Consumes: `store.panelists`, `store.live[id].confidence`, `store.live[id].status`, `store.live[id].stance`, `store.convergence` (`effectiveRuling`/`clusterFraction`/`meanConfidence`), `store.instantTokens`.
- Produces: `<OscilloscopeSync />` — self-contained, no props.

- [ ] **Step 1: Create `src/components/OscilloscopeSync.vue`**

```vue
<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useDeliberation } from "@/stores/deliberation";
import ConvergenceStrip from "@/components/ConvergenceStrip.vue";

const store = useDeliberation();
const canvas = ref<HTMLCanvasElement | null>(null);
let ctx: CanvasRenderingContext2D | null = null;
let raf = 0;
let phase = 0;
let ro: ResizeObserver | null = null;

// The live convergence signal — same data the retired glow computed read.
const signal = computed(() => {
  const c = store.convergence;
  const seats = store.panelists.map((p) => store.live[p.id]).filter(Boolean);
  const confs = seats.map((s) => s?.confidence ?? 0);
  const mean = c?.meanConfidence ?? (confs.length ? confs.reduce((a, b) => a + b, 0) / confs.length : 0);
  // Majority stance → phase-lock group; others ride out of phase.
  const counts = new Map<string, number>();
  for (const s of seats) if (s?.stance) counts.set(s.stance, (counts.get(s.stance) ?? 0) + 1);
  let majority = ""; let best = 0;
  for (const [k, v] of counts) if (v > best) { best = v; majority = k; }
  return {
    ruling: c?.effectiveRuling ?? "CONTINUE",
    cluster: c?.clusterFraction ?? 0,
    mean,
    channels: seats.map((s, i) => ({
      amp: 0.15 + (s?.confidence ?? 0) * 0.85,
      streaming: s?.status === "streaming",
      inGroup: !!s?.stance && s.stance === majority,
      idx: i,
    })),
  };
});

function color(): { line: string; glow: string } {
  const r = signal.value.ruling;
  if (r === "DEADLOCK") return { line: "oklch(0.62 0.25 22)", glow: "oklch(0.62 0.25 22 / 0.5)" };
  if (r === "CONSENSUS") return { line: "oklch(0.90 0.24 158)", glow: "oklch(0.90 0.24 158 / 0.6)" };
  return { line: "oklch(0.85 0.21 160)", glow: "oklch(0.85 0.21 160 / 0.4)" };
}

function resize() {
  const cv = canvas.value; if (!cv) return;
  const dpr = window.devicePixelRatio || 1;
  const w = cv.clientWidth; const h = cv.clientHeight;
  cv.width = Math.max(1, Math.round(w * dpr));
  cv.height = Math.max(1, Math.round(h * dpr));
  ctx?.setTransform(dpr, 0, 0, dpr, 0, 0);
}

function draw() {
  const cv = canvas.value; if (!cv || !ctx) return;
  const w = cv.clientWidth; const h = cv.clientHeight; const mid = h / 2;
  ctx.clearRect(0, 0, w, h);
  const s = signal.value; const c = color();
  const cluster = s.cluster; // 0..1 — how tightly channels fold into one line
  const noise = s.ruling === "DEADLOCK" ? 0.5 : (1 - cluster) * 0.25;
  ctx.lineWidth = 1.5;
  ctx.shadowBlur = 12; ctx.shadowColor = c.glow;

  const chans = s.channels.length ? s.channels : [{ amp: 0.2, streaming: false, inGroup: true, idx: 0 }];
  for (const ch of chans) {
    ctx.beginPath();
    // strokeStyle takes a single concrete color (no color-mix in canvas);
    // dim out-group channels via globalAlpha instead.
    ctx.strokeStyle = c.line;
    ctx.globalAlpha = ch.inGroup ? 0.95 : 0.4;
    // Grouped channels share phase; outliers offset. Streaming = jitter.
    const chanPhase = ch.inGroup ? 0 : (ch.idx + 1) * 1.3;
    const jitter = ch.streaming ? 0.18 : 0;
    const amp = ch.amp * mid * 0.7;
    const freq = 0.018 + ch.idx * 0.002;
    for (let x = 0; x <= w; x += 2) {
      const base = Math.sin(x * freq + phase + chanPhase);
      const n = (Math.sin(x * 0.13 + phase * 2.1 + ch.idx) * (noise + jitter));
      const y = mid - (base + n) * amp * (0.6 + s.mean * 0.4);
      x === 0 ? ctx.moveTo(x, y) : ctx.lineTo(x, y);
    }
    ctx.stroke();
  }
  ctx.globalAlpha = 1; ctx.shadowBlur = 0;
}

function frame() {
  phase += 0.05;
  draw();
  raf = requestAnimationFrame(frame);
}

function start() {
  cancelAnimationFrame(raf);
  if (store.instantTokens) { draw(); return; } // static single frame when reduced
  raf = requestAnimationFrame(frame);
}

onMounted(() => {
  ctx = canvas.value?.getContext("2d") ?? null;
  resize();
  ro = new ResizeObserver(() => { resize(); if (store.instantTokens) draw(); });
  if (canvas.value) ro.observe(canvas.value);
  start();
});
watch(() => store.instantTokens, start);
onBeforeUnmount(() => { cancelAnimationFrame(raf); ro?.disconnect(); });
</script>

<template>
  <section class="terminal-panel relative overflow-hidden" aria-label="Convergence oscilloscope">
    <div class="flex items-center justify-between px-3 pt-2 font-mono text-[9px] uppercase tracking-[0.14em] text-fg-faint">
      <span class="glow-text text-signal">◊ signal sync</span>
      <span v-if="signal.ruling === 'CONSENSUS'" class="glow-text text-consensus">▲ signal lock</span>
      <span v-else-if="signal.ruling === 'DEADLOCK'" class="glow-text text-deadlock glitch">✖ desync</span>
      <span v-else class="text-fg-faint">scanning…</span>
    </div>
    <canvas ref="canvas" class="block h-16 w-full" />
    <ConvergenceStrip class="px-3 pb-2" />
  </section>
</template>
```

- [ ] **Step 2: Mount the band in `src/screens/RoomScreen.vue`**

Import it and add it as the first row of `<main>`, above `MediatorPanel`. Change the template's `<main>` to:

```html
    <main class="grid min-h-0 grid-rows-[auto_auto_minmax(0,1fr)] gap-3 overflow-hidden">
      <OscilloscopeSync />
      <MediatorPanel />
      <section class="grid min-h-0 auto-rows-fr gap-3 overflow-y-auto pr-1" :class="columns"><SeatCard v-for="(seat, index) in store.panelists" :key="seat.id" :seat="seat" :index="index" /></section>
    </main>
```

And add the import in `<script setup>`:

```ts
import OscilloscopeSync from "@/components/OscilloscopeSync.vue";
```

- [ ] **Step 3: Remove `ConvergenceStrip` from the cockpit**

In `src/components/CockpitStatusBar.vue`: delete the `import ConvergenceStrip from "@/components/ConvergenceStrip.vue";` line and delete the `<ConvergenceStrip class="hidden lg:block" />` line from the template. (It now lives inside the oscilloscope.)

- [ ] **Step 4: Adjust `ConvergenceStrip.vue` for its new context**

It currently uses `text-signal`/`bg-signal` for the CONTINUE tone (post-rename) — that's correct. Only change: remove the `min-w-[12rem]` constraint so it fills the oscilloscope footer. In `src/components/ConvergenceStrip.vue`, change the root `<section class="min-w-[12rem]" …>` to `<section class="w-full" …>`.

- [ ] **Step 5: Typecheck**

```bash
npx vue-tsc --noEmit
```
Expected: passes.

- [ ] **Step 6: Verify the oscilloscope against the scripted stream**

`preview_start`, navigate `/?preview=stream`. Screenshot at ~3s (round 1, CONTINUE — loose green traces + "scanning…"), then `computer {action: "wait", duration: 8}` and screenshot (round 2 CONSENSUS — traces fold into one bright locked line + "signal lock"). Then navigate `/?preview=deadlock`, screenshot (red desync + "desync" glitch).
Expected: traces visibly tighten green as cluster→1; red noisy at deadlock. Then set `--effects-intensity: 0` via `javascript_tool` and confirm — actually verify reduced path: `javascript_tool` → `window.__store` isn't exposed, so instead reload with reduced motion: `resize_window {colorScheme:"dark"}` won't help. Verify static path by toggling effects Off in the UI (click the "Off" toggle in the top bar via `computer`/`find`), screenshot: the oscilloscope should freeze on a single frame (no animation), grid/glow gone.

- [ ] **Step 7: Commit**

```bash
git add src/components/OscilloscopeSync.vue src/screens/RoomScreen.vue src/components/CockpitStatusBar.vue src/components/ConvergenceStrip.vue
git commit -m "feat(room): oscilloscope sync signature moment (data-driven, effects-gated)"
```

---

## Task 7: Boot + convene handshake sequences

Replace the warm "Krunch / the deliberation chamber" boot overlay with a terminal boot/handshake, and add a brief "CONVENING…" cue when a session starts.

**Files:**
- Modify: `src/App.vue` (the boot overlay `<Transition>` block, template lines ~65-72)

**Interfaces:**
- Consumes: existing `booting` ref, `reduced()` helper, `store.running`.

- [ ] **Step 1: Replace the boot overlay markup**

Replace the boot overlay button block with a terminal handshake:

```html
    <Transition name="fade">
      <button v-if="booting" class="no-press absolute inset-0 z-50 grid place-items-center bg-bg-deep" @click="booting = false">
        <div class="boot text-left font-mono text-[11px] leading-5 text-signal">
          <p class="font-display glow-text text-4xl text-signal-bright">KRUNCH</p>
          <p class="mt-3 text-fg-faint">// deliberation ops terminal</p>
          <pre class="mt-4 text-fg-muted">&gt; initializing signal bus ......... <span class="text-signal">OK</span>
&gt; mounting panel channels ......... <span class="text-signal">OK</span>
&gt; calibrating oscilloscope ........ <span class="text-signal">OK</span>
&gt; handshake complete <span class="cursor">_</span></pre>
        </div>
      </button>
    </Transition>
```

- [ ] **Step 2: Verify the boot sequence**

`preview_start`, navigate `/` (fresh load). Screenshot within the boot window (the overlay auto-dismisses after ~520ms; if missed, `javascript_tool` → `location.reload()` and screenshot fast, or temporarily raise the timeout). Confirm the handshake renders green-on-black in Departure Mono, then fades to the app.
Expected: terminal boot readout, no warm serif wordmark.

- [ ] **Step 3: Commit**

```bash
git add src/App.vue
git commit -m "feat(boot): terminal handshake boot sequence"
```

---

## Task 8: Per-component polish sweep

Targeted class tweaks so key components read as HUD chrome. Structure/logic unchanged. Each sub-step is one file; screenshot after.

**Files:**
- Modify: `src/components/SeatCard.vue`, `src/components/CockpitStatusBar.vue`, `src/screens/VerdictScreen.vue`

**Interfaces:** none new — class-only edits.

- [ ] **Step 1: SeatCard → channel strip labels**

In `src/components/SeatCard.vue`, make the seat id a bracketed channel label and the header font mono. Change the id `<span>` (currently `class="mr-2 shrink-0 font-mono text-[11px] text-signal"`) to:

```html
<span class="mr-2 shrink-0 font-mono text-[11px] text-signal glow-text">[ CH{{ String(index + 1).padStart(2, '0') }} ]</span>
```

And change the display-name `<span>` from `font-display text-[15px]` to `font-sans text-[14px] tracking-wide` (Departure pixel is too heavy at 15px for a name). Change the streaming state label border color: in the `state` computed, the STREAM entry `["STREAM", "text-signal"]` stays; confirm ABSTAIN uses `text-deadlock`. No logic change.

- [ ] **Step 2: CockpitStatusBar → HUD rail + ticker**

In `src/components/CockpitStatusBar.vue`, replace the wordmark block:

```html
    <div class="flex items-center gap-2 text-signal glow-text"><Gavel class="size-4" /><span class="font-display text-base tracking-tight">KRUNCH</span></div>
```

and wrap the session/round readouts so they read as a ticker — change the round `<div>` to add `font-mono uppercase tracking-[0.1em]`. (Keep all existing logic, `elapsed`, `abort`, toggle intact.)

- [ ] **Step 3: VerdictScreen → "DECODED TRANSMISSION / RECORD"**

In `src/screens/VerdictScreen.vue`:
- Change the eyebrow `<p>` text from `The ruling` to `DECODED TRANSMISSION` and keep `text-signal` (now green).
- Change the pre-reveal `<pre>` boot text to:

```html
      <pre v-if="!ready" class="mt-5 overflow-hidden font-mono text-xs leading-6 text-fg-muted">&gt; decoding panel transmission…
&gt; reconciling final confidence
&gt; writing to record <span class="cursor">_</span></pre>
```
- The title `<p>` uses `font-display` (now Departure pixel) at `text-5xl sm:text-7xl` — reduce to `text-4xl sm:text-6xl` (pixel type is wider) and add `glow-text`. `meta` tones are already green/red/`text-signal` post-rename.
- Change `border-b`/frame: on the outer `<section class="terminal-panel … border-2 …">`, add `:class` binding so the frame is green on converge / red on deadlock — change to:
```html
    <section class="terminal-panel relative max-h-full w-full max-w-4xl overflow-y-auto border-2 p-6" :class="[ready ? 'boot' : '', store.finalState === 'deadlocked' ? 'border-deadlock' : 'border-signal']">
```

- [ ] **Step 4: Typecheck + full screenshot pass**

```bash
npx vue-tsc --noEmit
```
`preview_start`; screenshot `/?preview=stream` (room + oscilloscope + seat channel strips), `/?preview=verdict` (green DECODED TRANSMISSION), `/?preview=deadlock` (red frame).
Expected: cohesive HUD; channel labels `[ CH01 ]`; verdict reads as a decoded record.

- [ ] **Step 5: Commit**

```bash
git add src/components/SeatCard.vue src/components/CockpitStatusBar.vue src/screens/VerdictScreen.vue
git commit -m "feat(ui): HUD polish — channel strips, cockpit ticker, decoded-transmission verdict"
```

---

## Task 9: Rewrite `.impeccable.md` + final verification pass

Update the design-system doc to describe the new identity, and do a full acceptance pass across every effects mode and reduced-motion.

**Files:**
- Modify: `.impeccable.md`

- [ ] **Step 1: Rewrite `.impeccable.md`**

Replace the "Deliberation Chamber" content with the "Signal / ops-terminal" system: personality (live · high-stakes · instrumented), palette (green/red on black, token names), type (Departure/Neon/Krypton/Argon/Xenon roles), effects kit (phosphor glow, grid/dataflow, HUD chrome + glitch; NOT scanlines), the Oscilloscope Sync signature moment, and updated anti-references (warm/skeuomorphic/serif/candlelight). Keep the 5 design-principle structure but rewrite each for the terminal identity (e.g. principle 1 "The room is alive" → "The signal is live").

- [ ] **Step 2: Acceptance — effects modes**

`preview_start`, navigate `/?preview=stream`. Using the top-bar toggle (`find` "Ambient"/"Max"/"Off", `computer` click):
- **Max:** grid + glow + oscilloscope loud. Screenshot.
- **Ambient:** calmer. Screenshot.
- **Off:** all motion stopped, oscilloscope frozen on one frame, grid/glow gone, tokens instant. Screenshot + `read_console_messages {onlyErrors:true}` (expect none).

- [ ] **Step 3: Acceptance — reduced motion**

`resize_window` can't set reduced-motion; instead emulate via `javascript_tool` is unreliable for `matchMedia`. Verify the code path instead: confirm `store.instantTokens` gates the oscilloscope (Task 6 Step 1 `start()`), and confirm the CSS `@media (prefers-reduced-motion: reduce)` block at the bottom of `src/style.css` still zeroes animations (it is unchanged). Note in the commit that reduced-motion is covered by the existing media query + `instantTokens` gate.

- [ ] **Step 4: Acceptance — no stragglers**

```bash
grep -rn 'brass\|chamber-glow\|Young Serif\|Hanken\|candle' src ; echo "exit: $?"
```
Expected: no matches in active code (grep exit 1). `Young Serif`/`Hanken`/`Fragment Mono` `@font-face` in `fonts.css` may remain (files kept), but no component should reference them via `--font-*`. If a stray warm reference exists, fix it.

- [ ] **Step 5: Final typecheck + build**

```bash
npx vue-tsc --noEmit && npm run build
```
Expected: both pass.

- [ ] **Step 6: Commit**

```bash
git add .impeccable.md
git commit -m "docs(design): rewrite design system for the Signal ops-terminal identity"
```

---

## Self-Review notes (author)

- **Spec coverage:** palette (T3), type/Monaspace + font declarations (T1, T3), effects kit — phosphor/grid/dataflow/HUD/glitch, no scanlines (T4, T8), oscilloscope signature moment with real data mapping (T6), component reskins — SeatCard/Cockpit/Verdict/markdown (T5, T8), boot handshake (T7), `.impeccable.md` rewrite (T9), verification via `?preview` + effects modes + reduced-motion (T6, T9). Backend/store untouched per constraint.
- **Deviation from spec (intentional, flagged):** the spec placed the oscilloscope "where ConvergenceStrip is." `ConvergenceStrip` actually lives in the cockpit top bar (too small). The plan mounts the oscilloscope as a Room band and *reuses* `ConvergenceStrip` as its readout row, removing it from the cockpit — faithful to intent, correct on placement.
- **Type consistency:** `store.instantTokens`, `store.convergence.effectiveRuling/clusterFraction/meanConfidence`, `store.live[id].confidence/status/stance`, `store.panelists` all match [src/stores/deliberation.ts](../../../src/stores/deliberation.ts). `--font-prose`/`--font-record` are defined in T3 before use in T5/T6.
