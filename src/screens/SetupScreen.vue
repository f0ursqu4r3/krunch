<script setup lang="ts">
import { useDeliberation } from "@/stores/deliberation";
import SeatEditor from "@/components/SeatEditor.vue";

const store = useDeliberation();
</script>

<template>
  <div class="mx-auto max-w-4xl px-6 py-8">
    <header class="mb-6">
      <div class="flex items-center gap-3">
        <div class="flex size-10 items-center justify-center rounded-xl bg-primary/15 text-lg font-bold text-primary ring-1 ring-primary/30">
          k
        </div>
        <div>
          <h1 class="text-xl font-semibold tracking-tight">krunch</h1>
          <p class="text-xs text-muted-foreground">Lock a panel of LLMs in a room until consensus or deadlock.</p>
        </div>
        <button @click="store.loadDemoPanel()"
          class="ml-auto rounded-lg border px-3 py-1.5 text-xs text-muted-foreground hover:bg-accent hover:text-foreground">
          ⚡ Load demo panel (no keys)
        </button>
      </div>
    </header>

    <!-- Problem -->
    <section class="mb-6">
      <label class="mb-1.5 block text-sm font-medium">The problem</label>
      <textarea v-model="store.problem" rows="4" placeholder="What should the panel deliberate on?"
        class="w-full resize-y rounded-xl border bg-card px-4 py-3 text-sm outline-none focus:ring-2 focus:ring-ring" />
    </section>

    <!-- Settings -->
    <section class="mb-6 grid grid-cols-3 gap-4">
      <label class="text-sm">
        <span class="mb-1.5 block font-medium">Interaction mode</span>
        <select v-model="store.mode"
          class="w-full rounded-lg border bg-card px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-ring">
          <option value="autonomous">Autonomous — never pause</option>
          <option value="batched">Batched — pause when worth it</option>
          <option value="interactive">Interactive — pause on any question</option>
        </select>
      </label>
      <label class="text-sm">
        <span class="mb-1.5 block font-medium">Max rounds</span>
        <input v-model.number="store.maxRounds" type="number" min="1" max="64"
          class="w-full rounded-lg border bg-card px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-ring" />
      </label>
      <label class="text-sm">
        <span class="mb-1.5 block font-medium">Confidence floor</span>
        <input v-model.number="store.confidenceFloor" type="number" min="0" max="1" step="0.05"
          class="w-full rounded-lg border bg-card px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-ring" />
      </label>
    </section>

    <!-- Roster -->
    <section class="mb-6">
      <div class="mb-2 flex items-center justify-between">
        <h2 class="text-sm font-semibold">The panel</h2>
        <button @click="store.addPanelist()" :disabled="store.panelists.length >= 6"
          class="rounded-lg border px-3 py-1.5 text-xs hover:bg-accent disabled:opacity-40">
          + panelist ({{ store.panelists.length }}/6)
        </button>
      </div>
      <div class="space-y-3">
        <SeatEditor v-if="store.mediator" :seat="store.mediator" />
        <SeatEditor v-for="p in store.panelists" :key="p.id" :seat="p" removable @remove="store.removeSeat(p.id)" />
      </div>
    </section>

    <!-- Validation + start -->
    <div v-if="store.validation.length" class="mb-3 rounded-lg border border-amber-500/30 bg-amber-500/10 px-4 py-2 text-xs text-amber-300">
      <p v-for="(v, i) in store.validation" :key="i">• {{ v }}</p>
    </div>
    <div v-if="store.startError" class="mb-3 rounded-lg border border-destructive/30 bg-destructive/10 px-4 py-2 text-xs text-destructive">
      {{ store.startError }}
    </div>

    <button @click="store.start()" :disabled="store.validation.length > 0"
      class="w-full rounded-xl bg-primary py-3 text-sm font-semibold text-primary-foreground transition hover:opacity-90 disabled:opacity-40">
      Convene the panel
    </button>
  </div>
</template>
