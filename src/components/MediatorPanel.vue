<script setup lang="ts">
import { computed } from "vue";
import { useDeliberation } from "@/stores/deliberation";

const store = useDeliberation();
const last = computed(() => store.rounds[store.rounds.length - 1]);
const rulingCls: Record<string, string> = {
  CONSENSUS: "text-consensus border-consensus/40",
  CONTINUE: "text-brass border-brass/40",
  DEADLOCK: "text-deadlock border-deadlock/40",
};
</script>

<template>
  <div class="rise relative flex max-h-56 min-h-[9rem] flex-col overflow-hidden rounded-xl border border-brass/25 bg-surface/70"
    style="box-shadow: 0 0 60px -30px color-mix(in oklch, var(--brass) 70%, transparent);">
    <header class="flex items-center justify-between border-b border-brass/15 px-5 py-3">
      <div class="flex items-center gap-3">
        <span class="grid size-7 place-items-center rounded-full bg-brass/12 font-display text-brass ring-1 ring-brass/30">§</span>
        <div class="flex items-baseline gap-2.5">
          <span class="font-display text-base text-foreground">{{ store.mediator?.display_name ?? "The Foreman" }}</span>
          <span class="font-mono text-[10px] uppercase tracking-[0.14em] text-brass/60">mediator · at the head</span>
        </div>
      </div>
      <span v-if="last?.ruling"
        class="rounded-full border px-3 py-1 font-mono text-[10px] uppercase tracking-[0.16em]"
        :class="rulingCls[last.ruling]">
        {{ last.ruling }}<span v-if="last.downgraded" class="text-fg-faint"> · guard held</span>
      </span>
    </header>
    <div class="min-h-0 flex-1 overflow-y-auto px-5 py-3.5 text-[13.5px] leading-relaxed text-foreground/85">
      <p v-if="store.mediatorText" class="whitespace-pre-wrap break-words">{{ store.mediatorText }}</p>
      <p v-else class="font-display italic text-fg-faint">The foreman waits for the panel to speak, then weighs the room…</p>
    </div>
  </div>
</template>
