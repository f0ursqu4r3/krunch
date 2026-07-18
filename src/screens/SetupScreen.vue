<script setup lang="ts">
import { useDeliberation } from "@/stores/deliberation";
import SeatEditor from "@/components/SeatEditor.vue";

const store = useDeliberation();

const fieldCls =
  "w-full rounded-md border border-line bg-bg-deep/60 px-3 py-2 text-sm text-foreground outline-none transition focus:border-brass/60 focus:ring-1 focus:ring-brass/40";
</script>

<template>
  <div class="h-full overflow-y-auto">
    <div class="mx-auto max-w-3xl px-8 pb-24 pt-14">
      <!-- Wordmark -->
      <header class="rise flex items-start justify-between gap-6">
        <div class="flex items-center gap-4">
          <div class="relative grid size-14 place-items-center rounded-xl bg-surface-2 ring-1 ring-brass/30"
            style="box-shadow: 0 0 24px -6px color-mix(in oklch, var(--brass) 45%, transparent);">
            <span class="font-display text-2xl text-brass candle">k</span>
          </div>
          <div>
            <h1 class="font-display text-3xl leading-none text-foreground">krunch</h1>
            <p class="mt-1.5 text-sm text-fg-muted">
              Convene a panel of minds. Let them deliberate to a verdict.
            </p>
          </div>
        </div>
        <button @click="store.loadDemoPanel()"
          class="mt-1 shrink-0 rounded-full border border-line px-3.5 py-1.5 text-xs text-fg-muted transition hover:border-brass/50 hover:text-brass">
          Seat a demo panel
        </button>
      </header>

      <!-- The question -->
      <section class="rise mt-12" style="animation-delay: 60ms">
        <div class="mb-3 flex items-baseline gap-3">
          <span class="font-mono text-[11px] uppercase tracking-[0.2em] text-brass/70">i.</span>
          <h2 class="font-display text-lg text-foreground">The question before the panel</h2>
        </div>
        <textarea v-model="store.problem" rows="4"
          placeholder="State the matter to be deliberated…"
          class="w-full resize-none rounded-lg border border-line bg-surface/50 px-5 py-4 text-[15px] leading-relaxed text-foreground outline-none transition placeholder:text-fg-faint focus:border-brass/50 focus:bg-surface/80 focus:ring-1 focus:ring-brass/30" />
      </section>

      <!-- Chamber rules -->
      <section class="rise mt-10" style="animation-delay: 120ms">
        <div class="mb-3 flex items-baseline gap-3">
          <span class="font-mono text-[11px] uppercase tracking-[0.2em] text-brass/70">ii.</span>
          <h2 class="font-display text-lg text-foreground">Rules of the chamber</h2>
        </div>
        <div class="grid grid-cols-[1.6fr_1fr_1fr] gap-4">
          <label class="block">
            <span class="mb-1.5 block text-xs font-medium text-fg-muted">When to pause for you</span>
            <select v-model="store.mode" :class="fieldCls">
              <option value="autonomous">Autonomous — decide alone</option>
              <option value="batched">Batched — ask when it matters</option>
              <option value="interactive">Interactive — ask each round</option>
            </select>
          </label>
          <label class="block">
            <span class="mb-1.5 block text-xs font-medium text-fg-muted">Rounds before deadlock</span>
            <input v-model.number="store.maxRounds" type="number" min="1" max="64" :class="fieldCls" />
          </label>
          <label class="block">
            <span class="mb-1.5 block text-xs font-medium text-fg-muted">Confidence to rule</span>
            <input v-model.number="store.confidenceFloor" type="number" min="0" max="1" step="0.05" :class="fieldCls" />
          </label>
        </div>
      </section>

      <!-- The panel -->
      <section class="rise mt-10" style="animation-delay: 180ms">
        <div class="mb-4 flex items-center justify-between">
          <div class="flex items-baseline gap-3">
            <span class="font-mono text-[11px] uppercase tracking-[0.2em] text-brass/70">iii.</span>
            <h2 class="font-display text-lg text-foreground">The panel</h2>
          </div>
          <button @click="store.addPanelist()" :disabled="store.panelists.length >= 6"
            class="rounded-full border border-line px-3.5 py-1.5 text-xs text-fg-muted transition hover:border-brass/50 hover:text-brass disabled:cursor-not-allowed disabled:opacity-40">
            Seat another
            <span class="ml-1 font-mono text-fg-faint">{{ store.panelists.length }}/6</span>
          </button>
        </div>
        <div class="space-y-3">
          <SeatEditor v-if="store.mediator" :seat="store.mediator" />
          <SeatEditor v-for="p in store.panelists" :key="p.id" :seat="p" removable @remove="store.removeSeat(p.id)" />
        </div>
      </section>

      <!-- Objections -->
      <div v-if="store.validation.length" class="rise mt-8 rounded-lg bg-surface/40 p-4 ring-1 ring-deadlock/25">
        <p class="mb-1.5 font-mono text-[11px] uppercase tracking-[0.15em] text-deadlock">Before you convene</p>
        <ul class="space-y-0.5 text-sm text-fg-muted">
          <li v-for="(v, i) in store.validation" :key="i">— {{ v }}</li>
        </ul>
      </div>
      <div v-if="store.startError" class="mt-4 rounded-lg bg-danger/10 px-4 py-2.5 text-sm text-danger ring-1 ring-danger/25">
        {{ store.startError }}
      </div>

      <!-- Convene -->
      <button @click="store.start()" :disabled="store.validation.length > 0"
        class="group mt-8 flex w-full items-center justify-center gap-3 rounded-xl bg-brass py-4 font-display text-lg text-primary-foreground transition hover:bg-brass-bright disabled:cursor-not-allowed disabled:bg-surface-3 disabled:text-fg-faint"
        style="box-shadow: 0 8px 40px -12px color-mix(in oklch, var(--brass) 60%, transparent);">
        <span>Convene the panel</span>
        <span class="transition-transform group-hover:translate-x-0.5" aria-hidden>→</span>
      </button>
    </div>
  </div>
</template>
