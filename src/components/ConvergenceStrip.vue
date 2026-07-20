<script setup lang="ts">
import { computed } from "vue";
import { useDeliberation } from "@/stores/deliberation";

const store = useDeliberation();
const telemetry = computed(() => {
  if (store.convergence) return store.convergence;
  const last = store.rounds[store.rounds.length - 1];
  return last ? { round: last.round, effectiveRuling: last.ruling ?? "CONTINUE", clusterFraction: last.clusterFraction ?? 0, meanConfidence: last.meanConfidence ?? 0, downgraded: last.downgraded ?? false } : null;
});
const tone = computed(() => telemetry.value?.effectiveRuling === "CONSENSUS" ? "text-consensus" : telemetry.value?.effectiveRuling === "DEADLOCK" ? "text-deadlock" : "text-brass");
</script>

<template>
  <section class="min-w-[12rem]" aria-label="Convergence telemetry">
    <div class="mb-1 flex items-center justify-between gap-2 font-mono text-[9px] uppercase tracking-[0.12em] text-fg-faint">
      <span>convergence</span><span :class="tone">{{ telemetry?.effectiveRuling ?? "PENDING" }}</span>
    </div>
    <div class="flex items-center gap-2">
      <div class="h-2 flex-1 overflow-hidden border border-line bg-bg-deep">
        <div class="h-full transition-[width] duration-300" :class="telemetry?.effectiveRuling === 'DEADLOCK' ? 'bg-deadlock shake' : telemetry?.effectiveRuling === 'CONSENSUS' ? 'bg-consensus' : 'bg-brass'" :style="{ width: `${Math.round((telemetry?.clusterFraction ?? 0) * 100)}%` }" />
      </div>
      <span class="w-9 text-right font-mono text-[10px]" :class="tone">{{ Math.round((telemetry?.clusterFraction ?? 0) * 100) }}%</span>
    </div>
    <p class="mt-1 font-mono text-[9px] text-fg-faint">cluster / μ confidence {{ Math.round((telemetry?.meanConfidence ?? 0) * 100) }}<span v-if="telemetry?.downgraded"> / guard</span></p>
  </section>
</template>
