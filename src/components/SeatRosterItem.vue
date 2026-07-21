<script setup lang="ts">
import { computed } from "vue";
import type { SeatConfig } from "@/lib/types";
import { personaLabels } from "@/lib/personas";

const props = defineProps<{ seat: SeatConfig; selected?: boolean }>();
defineEmits<{ select: [] }>();

const isMediator = computed(() => props.seat.role === "mediator");
const chips = computed(() => personaLabels(props.seat.personas));
const providerLabel = computed(() => ({
  anthropic: "anthropic", open_ai_compatible: "openai", claude_cli: "claude cli", codex_cli: "codex cli", demo: "demo",
}[props.seat.provider]));
const modelLine = computed(() => (props.seat.provider === "demo" ? "demo" : `${providerLabel.value} · ${props.seat.model}`));
</script>

<template>
  <button type="button" :aria-pressed="selected" @click="$emit('select')"
    class="w-full rounded-lg border border-l-2 border-line p-3 text-left transition-colors hover:border-line-strong"
    :class="selected ? 'border-line-strong border-l-signal bg-surface' : 'border-l-transparent bg-bg-deep/60'">
    <div class="flex items-center gap-2">
      <span class="rounded border px-1.5 py-0.5 font-mono text-[9px] tracking-wide" :class="isMediator ? 'border-signal-deep/60 text-signal' : 'border-line-strong text-fg-faint'">{{ isMediator ? 'MED' : 'SEAT' }}</span>
      <span class="min-w-0 flex-1 truncate font-display text-sm text-foreground">{{ seat.display_name }}</span>
    </div>
    <div class="mt-2 flex flex-wrap items-center gap-1.5">
      <span v-for="chip in chips" :key="chip" class="rounded-full bg-signal/15 px-2 py-0.5 font-mono text-[10px] text-signal-bright">{{ chip }}</span>
      <span class="font-mono text-[10px] text-fg-faint">{{ chips.length ? '· ' : '' }}{{ modelLine }}</span>
    </div>
  </button>
</template>
