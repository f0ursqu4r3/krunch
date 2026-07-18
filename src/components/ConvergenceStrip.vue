<script setup lang="ts">
import { computed } from "vue";
import { useDeliberation } from "@/stores/deliberation";

const store = useDeliberation();

// Stable panelist ordering for the rows.
const panelistIds = computed(() => store.panelists.map((p) => p.id));
function nameOf(id: string) {
  return store.seats.find((s) => s.id === id)?.display_name ?? id.slice(0, 6);
}
function confidenceIn(round: { stances: { seat: string; confidence: number }[] }, seat: string) {
  return round.stances.find((s) => s.seat === seat)?.confidence ?? 0;
}
const rulingColor: Record<string, string> = {
  CONSENSUS: "bg-emerald-500",
  CONTINUE: "bg-primary",
  DEADLOCK: "bg-destructive",
};
</script>

<template>
  <div class="rounded-xl border bg-card p-3">
    <div class="mb-2 flex items-center justify-between">
      <h3 class="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Convergence</h3>
      <span class="text-[10px] text-muted-foreground">confidence per round</span>
    </div>

    <div v-if="store.rounds.length === 0" class="py-4 text-center text-xs text-muted-foreground">
      no completed rounds yet
    </div>

    <div v-else class="overflow-x-auto">
      <table class="w-full border-separate border-spacing-1 text-[11px]">
        <thead>
          <tr>
            <th class="w-24 text-left font-normal text-muted-foreground"></th>
            <th v-for="r in store.rounds" :key="r.round" class="text-center font-normal text-muted-foreground">
              R{{ r.round + 1 }}
            </th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="pid in panelistIds" :key="pid">
            <td class="truncate pr-1 text-muted-foreground">{{ nameOf(pid) }}</td>
            <td v-for="r in store.rounds" :key="r.round" class="text-center">
              <div class="mx-auto h-6 w-6 rounded" :style="{
                backgroundColor: `color-mix(in srgb, var(--primary) ${confidenceIn(r, pid) * 100}%, var(--muted))`,
              }" :title="`${(confidenceIn(r, pid) * 100).toFixed(0)}%`" />
            </td>
          </tr>
          <tr>
            <td class="pr-1 text-muted-foreground">ruling</td>
            <td v-for="r in store.rounds" :key="r.round" class="text-center">
              <span class="mx-auto inline-block h-2 w-6 rounded-full" :class="rulingColor[r.ruling ?? ''] ?? 'bg-muted'"
                :title="r.ruling" />
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>
