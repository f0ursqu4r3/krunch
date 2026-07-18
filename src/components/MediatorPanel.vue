<script setup lang="ts">
import { computed } from "vue";
import { useDeliberation } from "@/stores/deliberation";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";

const store = useDeliberation();
const last = computed(() => store.rounds[store.rounds.length - 1]);
const rulingCls: Record<string, string> = {
  CONSENSUS: "border-consensus/40 text-consensus",
  CONTINUE: "border-brass/40 text-brass",
  DEADLOCK: "border-deadlock/40 text-deadlock",
};
</script>

<template>
  <Card class="rise max-h-56 min-h-[9rem] gap-0 overflow-hidden border-brass/25 bg-surface/70 py-0"
    style="box-shadow: 0 0 60px -30px color-mix(in oklch, var(--brass) 70%, transparent);">
    <CardHeader class="flex items-center justify-between border-b border-brass/15 px-5 py-3">
      <div class="flex items-center gap-3">
        <span class="grid size-7 place-items-center rounded-full bg-brass/12 font-display text-brass ring-1 ring-brass/30">§</span>
        <div class="flex items-baseline gap-2.5">
          <span class="font-display text-base text-foreground">{{ store.mediator?.display_name ?? "The Foreman" }}</span>
          <span class="font-mono text-[10px] uppercase tracking-[0.14em] text-brass/60">mediator · at the head</span>
        </div>
      </div>
      <Badge v-if="last?.ruling" variant="outline"
        class="rounded-full font-mono text-[10px] uppercase tracking-[0.16em]" :class="rulingCls[last.ruling]">
        {{ last.ruling }}<span v-if="last.downgraded" class="text-fg-faint">&nbsp;· guard held</span>
      </Badge>
    </CardHeader>
    <CardContent class="min-h-0 flex-1 overflow-y-auto px-5 py-3.5 text-[13.5px] leading-relaxed text-foreground/85">
      <p v-if="store.mediatorText" class="whitespace-pre-wrap break-words">{{ store.mediatorText }}</p>
      <p v-else class="font-display italic text-fg-faint">The foreman waits for the panel to speak, then weighs the room…</p>
    </CardContent>
  </Card>
</template>
