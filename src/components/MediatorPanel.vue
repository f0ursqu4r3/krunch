<script setup lang="ts">
import { computed } from "vue";
import { useDeliberation } from "@/stores/deliberation";

const store = useDeliberation();
const lastRuling = computed(() => store.rounds[store.rounds.length - 1]);
</script>

<template>
  <div class="flex min-h-0 flex-col rounded-xl border bg-card ring-1 ring-primary/20">
    <header class="flex items-center justify-between border-b px-4 py-2.5">
      <div class="flex items-center gap-2">
        <span class="text-lg">⚖️</span>
        <span class="text-sm font-semibold">{{ store.mediator?.display_name ?? "Mediator" }}</span>
        <span class="text-xs text-muted-foreground">foreman</span>
      </div>
      <span v-if="lastRuling?.ruling" class="rounded-full px-2.5 py-0.5 text-[11px] font-semibold ring-1"
        :class="{
          'bg-emerald-500/15 text-emerald-400 ring-emerald-500/30': lastRuling.ruling === 'CONSENSUS',
          'bg-primary/15 text-primary ring-primary/30': lastRuling.ruling === 'CONTINUE',
          'bg-destructive/15 text-destructive ring-destructive/30': lastRuling.ruling === 'DEADLOCK',
        }">
        {{ lastRuling.ruling }}<span v-if="lastRuling.downgraded"> · downgraded</span>
      </span>
    </header>
    <div class="min-h-0 flex-1 overflow-y-auto px-4 py-3 text-[13px] leading-relaxed">
      <p v-if="store.mediatorText" class="whitespace-pre-wrap break-words text-card-foreground/90">{{ store.mediatorText }}</p>
      <p v-else class="italic text-muted-foreground">The mediator will summarize once the panel has spoken…</p>
    </div>
  </div>
</template>
