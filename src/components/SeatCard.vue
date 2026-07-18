<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useDeliberation } from "@/stores/deliberation";
import type { SeatConfig } from "@/lib/types";
import { measure } from "@/lib/pretext";

const props = defineProps<{ seat: SeatConfig }>();
const store = useDeliberation();

const l = computed(() => store.live[props.seat.id]);
const lineCount = ref(0);

// Reflow-free measurement of the streamed text (Pretext).
watch(
  () => l.value?.text,
  async (t) => {
    if (!t) {
      lineCount.value = 0;
      return;
    }
    const m = await measure(t, 320);
    lineCount.value = m.lineCount;
  },
);

const statusMeta = computed(() => {
  switch (l.value?.status) {
    case "streaming":
      return { label: "streaming", cls: "bg-primary/15 text-primary ring-primary/30" };
    case "stance":
      return { label: "stance in", cls: "bg-emerald-500/15 text-emerald-400 ring-emerald-500/30" };
    case "abstained":
      return { label: "abstained", cls: "bg-destructive/15 text-destructive ring-destructive/30" };
    case "truncated":
      return { label: "truncated", cls: "bg-amber-500/15 text-amber-400 ring-amber-500/30" };
    default:
      return { label: "idle", cls: "bg-muted text-muted-foreground ring-border" };
  }
});
</script>

<template>
  <div class="flex min-h-0 flex-col rounded-xl border bg-card">
    <header class="flex items-center justify-between gap-2 border-b px-3 py-2">
      <div class="flex items-center gap-2 truncate">
        <span class="size-2 shrink-0 rounded-full" :class="l?.status === 'streaming' ? 'bg-primary animate-pulse' : 'bg-muted-foreground/40'" />
        <span class="truncate text-sm font-medium">{{ seat.display_name }}</span>
        <span class="truncate text-xs text-muted-foreground">{{ seat.model }}</span>
      </div>
      <span class="shrink-0 rounded-full px-2 py-0.5 text-[10px] font-medium ring-1" :class="statusMeta.cls">
        {{ statusMeta.label }}
      </span>
    </header>

    <div class="min-h-0 flex-1 overflow-y-auto px-3 py-2 text-[13px] leading-relaxed text-card-foreground/90">
      <p v-if="l?.text" class="whitespace-pre-wrap break-words">{{ l.text }}</p>
      <p v-else class="text-muted-foreground italic">waiting…</p>
      <p v-if="l?.status === 'abstained'" class="mt-2 text-xs text-destructive">
        abstained — {{ l.reason }}
      </p>
    </div>

    <footer v-if="l?.stance" class="border-t px-3 py-2">
      <p class="text-xs font-medium text-foreground">{{ l.stance }}</p>
      <div class="mt-1.5 flex items-center gap-2">
        <div class="h-1.5 flex-1 overflow-hidden rounded-full bg-muted">
          <div class="h-full rounded-full bg-primary transition-all" :style="{ width: `${(l.confidence ?? 0) * 100}%` }" />
        </div>
        <span class="w-8 text-right text-[11px] tabular-nums text-muted-foreground">
          {{ ((l.confidence ?? 0) * 100).toFixed(0) }}%
        </span>
      </div>
    </footer>
    <footer v-else-if="lineCount > 0" class="border-t px-3 py-1 text-right text-[10px] text-muted-foreground/60">
      {{ lineCount }} lines
    </footer>
  </div>
</template>
