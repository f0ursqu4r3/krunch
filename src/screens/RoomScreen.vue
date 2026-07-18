<script setup lang="ts">
import { computed } from "vue";
import { useDeliberation } from "@/stores/deliberation";
import MediatorPanel from "@/components/MediatorPanel.vue";
import SeatCard from "@/components/SeatCard.vue";
import EventLogRail from "@/components/EventLogRail.vue";

const store = useDeliberation();
const columns = computed(() => store.panelists.length <= 2 ? "grid-cols-2" : store.panelists.length <= 4 ? "grid-cols-2" : "grid-cols-3");
</script>

<template>
  <div class="grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_19rem] gap-3 overflow-hidden p-3">
    <main class="grid min-h-0 grid-rows-[auto_minmax(0,1fr)] gap-3 overflow-hidden">
      <MediatorPanel />
      <section class="grid min-h-0 auto-rows-fr gap-3 overflow-y-auto pr-1" :class="columns"><SeatCard v-for="(seat, index) in store.panelists" :key="seat.id" :seat="seat" :index="index" /></section>
    </main>
    <EventLogRail />
  </div>
</template>
