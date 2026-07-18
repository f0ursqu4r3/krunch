<script setup lang="ts">
import { computed } from "vue";
import { useDeliberation } from "@/stores/deliberation";
import SeatCard from "@/components/SeatCard.vue";
import MediatorPanel from "@/components/MediatorPanel.vue";
import ConvergenceStrip from "@/components/ConvergenceStrip.vue";
import AwaitingUserModal from "@/components/AwaitingUserModal.vue";

const store = useDeliberation();
const progress = computed(() =>
  Math.min(100, ((store.currentRound + 1) / store.maxRounds) * 100),
);
const gridCols = computed(() => {
  const n = store.panelists.length;
  return n <= 2 ? "grid-cols-2" : n <= 4 ? "grid-cols-2" : "grid-cols-3";
});
</script>

<template>
  <div class="flex h-full flex-col">
    <!-- top bar -->
    <header class="flex items-center justify-between border-b px-6 py-3">
      <div class="flex items-center gap-3">
        <span class="text-sm font-semibold">Deliberation</span>
        <span class="text-xs text-muted-foreground">round {{ store.currentRound + 1 }} / {{ store.maxRounds }}</span>
      </div>
      <div class="flex items-center gap-3">
        <div class="h-1.5 w-40 overflow-hidden rounded-full bg-muted">
          <div class="h-full rounded-full bg-primary transition-all" :style="{ width: `${progress}%` }" />
        </div>
        <button @click="store.abandon()" class="rounded-lg border px-3 py-1.5 text-xs text-muted-foreground hover:text-destructive">
          Abandon
        </button>
      </div>
    </header>

    <!-- body -->
    <div class="grid min-h-0 flex-1 grid-cols-[1fr_320px] gap-4 p-4">
      <!-- left: panel + mediator -->
      <div class="grid min-h-0 grid-rows-[1fr_auto] gap-4">
        <div class="grid min-h-0 gap-3" :class="gridCols">
          <SeatCard v-for="p in store.panelists" :key="p.id" :seat="p" />
        </div>
        <div class="h-56">
          <MediatorPanel />
        </div>
      </div>

      <!-- right: convergence -->
      <aside class="min-h-0 overflow-y-auto">
        <ConvergenceStrip />
      </aside>
    </div>

    <AwaitingUserModal />
  </div>
</template>
