<script setup lang="ts">
import { computed, ref, watch } from "vue";
import type { SeatConfig } from "@/lib/types";
import { useDeliberation } from "@/stores/deliberation";
import { measure } from "@/lib/pretext";

const props = defineProps<{ seat: SeatConfig; index: number }>();
const store = useDeliberation(); const lines = ref(0);
const live = computed(() => store.live[props.seat.id]);
watch(() => live.value?.text, async (text) => lines.value = text ? (await measure(text, 320)).lineCount : 0);
const state = computed(() => ({
  streaming: ["STREAM", "text-brass"], stance: ["LATCHED", "text-consensus"], abstained: ["ABSTAIN", "text-deadlock"], truncated: ["TRUNC", "text-brass"], idle: ["IDLE", "text-fg-faint"],
}[live.value?.status ?? "idle"]));
const ttft = computed(() => live.value?.firstTokenAt && live.value.startedAt ? `~${live.value.firstTokenAt - live.value.startedAt}ms TTFT` : "~— TTFT");
</script>

<template>
  <article :id="`seat-${index + 1}`" :data-seat-index="index" tabindex="-1" class="terminal-panel flex min-h-[12rem] min-w-0 flex-col overflow-hidden outline-none focus-visible:ring-2 focus-visible:ring-cyan" :class="[live?.status === 'stance' ? 'border-consensus latch' : live?.status === 'abstained' ? 'border-deadlock' : live?.status === 'streaming' ? 'border-brass' : '', live?.streamIncomplete ? 'shake' : '']">
    <header class="flex items-center justify-between gap-2 border-b border-line bg-bg-deep/40 px-3 py-2">
      <div class="min-w-0"><span class="mr-2 text-cyan">[{{ String(index + 1).padStart(2, '0') }}]</span><span class="truncate font-mono text-xs text-foreground">{{ seat.display_name }}</span></div>
      <span class="shrink-0 font-mono text-[9px]" :class="state[1]">{{ state[0] }}</span>
    </header>
    <div class="min-h-0 flex-1 overflow-y-auto px-3 py-3 text-[11px] leading-[1.7] text-foreground/90">
      <p v-if="live?.text" class="whitespace-pre-wrap break-words">{{ live.text }}<span v-if="live.status === 'streaming'" class="cursor text-brass">▋</span></p>
      <p v-else class="text-fg-faint">&gt; awaiting stream<span class="cursor">_</span></p>
      <p v-if="live?.reason" class="mt-2 text-deadlock">! {{ live.reason }}</p>
    </div>
    <footer class="border-t border-line px-3 py-2 font-mono text-[9px] text-fg-faint">
      <div class="flex justify-between gap-2"><span>{{ ttft }}</span><span>{{ live?.usage?.outputTokens === null || !live?.usage ? '~— tok/s' : `~${live.usage.outputTokens} out` }}</span></div>
      <div class="mt-1 flex justify-between gap-2"><span>{{ lines }} lines · {{ live?.usage ? `${live.usage.inputTokens ?? '?'} in / ${live.usage.outputTokens ?? '?'} out` : 'usage pending' }}</span><span v-if="live?.streamIncomplete" class="text-deadlock">⚠ stream incomplete</span></div>
      <p v-if="live?.stance" class="mt-2 border-t border-consensus/35 pt-2 text-consensus">STANCE: {{ live.stance }} // {{ Math.round((live.confidence ?? 0) * 100) }}%</p>
    </footer>
  </article>
</template>
