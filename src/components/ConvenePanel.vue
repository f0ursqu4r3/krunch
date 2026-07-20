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
