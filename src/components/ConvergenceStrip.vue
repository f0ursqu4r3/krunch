<script setup lang="ts">
import { computed } from "vue";
import { useDeliberation } from "@/stores/deliberation";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Empty, EmptyDescription, EmptyHeader, EmptyTitle } from "@/components/ui/empty";

const store = useDeliberation();
const panelistIds = computed(() => store.panelists.map((p) => p.id));
function nameOf(id: string) {
  return store.seats.find((s) => s.id === id)?.display_name ?? id.slice(0, 6);
}
function conf(round: { stances: { seat: string; confidence: number }[] }, seat: string) {
  return round.stances.find((s) => s.seat === seat)?.confidence ?? 0;
}
const rulingDot: Record<string, string> = {
  CONSENSUS: "bg-consensus",
  CONTINUE: "bg-brass/60",
  DEADLOCK: "bg-deadlock",
};
</script>

<template>
  <Card class="rise h-full gap-0 border-line bg-surface/40 py-0" style="animation-delay: 120ms">
    <CardHeader class="border-b border-line/60 px-4 py-3">
      <h3 class="font-display text-sm text-foreground">The tally</h3>
      <p class="mt-0.5 font-mono text-[10px] uppercase tracking-[0.14em] text-fg-faint">conviction by round</p>
    </CardHeader>

    <Empty v-if="store.rounds.length === 0" class="flex-1">
      <EmptyHeader>
        <EmptyTitle class="font-display text-sm italic text-fg-faint">No rounds have closed</EmptyTitle>
        <EmptyDescription class="text-fg-faint/70">The room is still speaking.</EmptyDescription>
      </EmptyHeader>
    </Empty>

    <CardContent v-else class="flex-1 overflow-auto p-3">
      <table class="w-full border-separate border-spacing-1.5 text-[11px]">
        <thead>
          <tr>
            <th></th>
            <th v-for="r in store.rounds" :key="r.round"
              class="font-mono text-[10px] font-normal text-fg-faint">R{{ r.round + 1 }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="pid in panelistIds" :key="pid">
            <td class="max-w-[90px] truncate pr-1 text-fg-muted">{{ nameOf(pid) }}</td>
            <td v-for="r in store.rounds" :key="r.round" class="text-center">
              <div class="mx-auto grid size-7 place-items-center rounded"
                :style="{ backgroundColor: `color-mix(in oklch, var(--brass) ${conf(r, pid) * 90 + 6}%, var(--surface-3))` }"
                :title="`${(conf(r, pid) * 100).toFixed(0)}%`">
                <span class="font-mono text-[9px]"
                  :style="{ color: conf(r, pid) > 0.5 ? 'var(--brass-ink)' : 'var(--fg-faint)' }">
                  {{ (conf(r, pid) * 100).toFixed(0) }}
                </span>
              </div>
            </td>
          </tr>
          <tr>
            <td class="pr-1 font-mono text-[10px] uppercase tracking-wide text-fg-faint">rule</td>
            <td v-for="r in store.rounds" :key="r.round" class="text-center">
              <span class="mx-auto block h-1.5 w-6 rounded-full" :class="rulingDot[r.ruling ?? ''] ?? 'bg-surface-3'"
                :title="r.ruling" />
            </td>
          </tr>
        </tbody>
      </table>
    </CardContent>
  </Card>
</template>
