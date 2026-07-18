<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useDeliberation } from "@/stores/deliberation";
import type { SeatConfig } from "@/lib/types";
import { measure } from "@/lib/pretext";

const props = defineProps<{ seat: SeatConfig; index?: number }>();
const store = useDeliberation();

const l = computed(() => store.live[props.seat.id]);
const lineCount = ref(0);
const active = computed(() => l.value?.status === "streaming");

watch(
  () => l.value?.text,
  async (t) => {
    if (!t) return (lineCount.value = 0);
    lineCount.value = (await measure(t, 320)).lineCount;
  },
);

const status = computed(() => {
  switch (l.value?.status) {
    case "streaming": return { label: "speaking", cls: "text-brass" };
    case "stance": return { label: "position filed", cls: "text-consensus" };
    case "abstained": return { label: "abstained", cls: "text-deadlock" };
    case "truncated": return { label: "cut short", cls: "text-brass" };
    default: return { label: "listening", cls: "text-fg-faint" };
  }
});
</script>

<template>
  <div class="rise flex min-h-0 flex-col overflow-hidden rounded-xl border bg-surface/60 transition-colors duration-500"
    :class="active ? 'border-brass/40' : 'border-line'"
    :style="[
      { animationDelay: `${(index ?? 0) * 80}ms` },
      active ? { boxShadow: '0 0 40px -16px color-mix(in oklch, var(--brass) 55%, transparent)' } : {},
    ]">
    <header class="flex items-center justify-between gap-2 border-b border-line/60 px-4 py-2.5">
      <div class="flex min-w-0 items-center gap-2.5">
        <span class="size-1.5 shrink-0 rounded-full transition-colors"
          :class="active ? 'bg-brass candle' : 'bg-fg-faint/40'" />
        <span class="truncate font-display text-[15px] text-foreground">{{ seat.display_name }}</span>
      </div>
      <span class="shrink-0 font-mono text-[10px] uppercase tracking-[0.12em]" :class="status.cls">
        {{ status.label }}
      </span>
    </header>

    <div class="min-h-0 flex-1 overflow-y-auto px-4 py-3 text-[13px] leading-relaxed text-foreground/85">
      <p v-if="l?.text" class="whitespace-pre-wrap break-words">{{ l.text }}</p>
      <p v-else class="font-display text-sm italic text-fg-faint">awaiting the floor…</p>
      <p v-if="l?.status === 'abstained'" class="mt-2 font-mono text-[11px] text-deadlock/90">
        abstained — {{ l.reason }}
      </p>
    </div>

    <footer v-if="l?.stance" class="border-t border-line/60 bg-bg-deep/30 px-4 py-3">
      <p class="text-[13px] font-medium leading-snug text-foreground">"{{ l.stance }}"</p>
      <div class="mt-2 flex items-center gap-2.5">
        <span class="font-mono text-[10px] uppercase tracking-wider text-fg-faint">conviction</span>
        <div class="h-[3px] flex-1 overflow-hidden rounded-full bg-surface-3">
          <div class="h-full rounded-full bg-brass transition-all duration-700"
            :style="{ width: `${(l.confidence ?? 0) * 100}%` }" />
        </div>
        <span class="w-8 text-right font-mono text-[11px] text-brass">
          {{ ((l.confidence ?? 0) * 100).toFixed(0) }}
        </span>
      </div>
    </footer>
    <footer v-else-if="lineCount > 0" class="border-t border-line/40 px-4 py-1.5 text-right">
      <span class="font-mono text-[10px] text-fg-faint/60">{{ lineCount }} lines spoken</span>
    </footer>
  </div>
</template>
