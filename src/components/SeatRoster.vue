<script setup lang="ts">
import type { SeatConfig } from "@/lib/types";
import SeatRosterItem from "./SeatRosterItem.vue";

defineProps<{ seats: SeatConfig[]; selectedId: string | null; canAdd: boolean }>();
defineEmits<{ select: [id: string]; add: [] }>();
</script>

<template>
  <div class="flex flex-col gap-2">
    <SeatRosterItem v-for="seat in seats" :key="seat.id" :seat="seat" :selected="seat.id === selectedId" @select="$emit('select', seat.id)" />
    <button type="button" :disabled="!canAdd" @click="$emit('add')"
      class="rounded-lg border border-dashed border-line p-2.5 text-center font-mono text-[11px] text-fg-faint transition-colors hover:border-signal/50 hover:text-signal disabled:opacity-40 disabled:hover:border-line disabled:hover:text-fg-faint">
      + Add seat <kbd class="opacity-60">A</kbd>
    </button>
  </div>
</template>
