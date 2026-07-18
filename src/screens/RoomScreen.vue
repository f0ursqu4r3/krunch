<script setup lang="ts">
import { computed } from "vue";
import { useDeliberation } from "@/stores/deliberation";
import SeatCard from "@/components/SeatCard.vue";
import MediatorPanel from "@/components/MediatorPanel.vue";
import ConvergenceStrip from "@/components/ConvergenceStrip.vue";
import AwaitingUserModal from "@/components/AwaitingUserModal.vue";
import { Button } from "@/components/ui/button";

const store = useDeliberation();

const gridCols = computed(() => {
  const n = store.panelists.length;
  return n <= 2 ? "grid-cols-2" : n === 3 ? "grid-cols-3" : n <= 4 ? "grid-cols-2" : "grid-cols-3";
});
const rounds = computed(() => Array.from({ length: store.maxRounds }, (_, i) => i));
</script>

<template>
  <div class="flex h-full flex-col">
    <!-- Chamber header -->
    <header class="flex items-center justify-between gap-6 border-b border-line/70 px-7 py-4">
      <div class="flex items-baseline gap-4">
        <h1 class="font-display text-xl text-foreground">In deliberation</h1>
        <span class="font-mono text-xs text-fg-faint">
          round <span class="text-brass">{{ String(store.currentRound + 1).padStart(2, "0") }}</span>
          / {{ String(store.maxRounds).padStart(2, "0") }}
        </span>
      </div>

      <div class="flex items-center gap-5">
        <!-- Round candles -->
        <div class="flex items-center gap-1.5">
          <span v-for="r in rounds" :key="r"
            class="h-1.5 rounded-full transition-all duration-500"
            :class="[
              r < store.currentRound ? 'w-1.5 bg-brass/50' :
              r === store.currentRound ? 'w-6 bg-brass candle' : 'w-1.5 bg-surface-3',
            ]" />
        </div>
        <Button variant="outline" size="sm" @click="store.abandon()"
          class="rounded-full border-line text-fg-muted hover:border-deadlock/50 hover:text-deadlock">
          Adjourn
        </Button>
      </div>
    </header>

    <!-- The chamber floor -->
    <div class="grid min-h-0 flex-1 grid-cols-[1fr_300px] gap-5 p-5">
      <div class="grid min-h-0 grid-rows-[auto_1fr] gap-5">
        <!-- Mediator at the head of the table -->
        <MediatorPanel />
        <!-- The panel, seated -->
        <div class="grid min-h-0 gap-4" :class="gridCols">
          <SeatCard v-for="(p, i) in store.panelists" :key="p.id" :seat="p" :index="i" />
        </div>
      </div>

      <!-- The tally -->
      <aside class="min-h-0 overflow-y-auto">
        <ConvergenceStrip />
      </aside>
    </div>

    <AwaitingUserModal />
  </div>
</template>
