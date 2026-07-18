<script setup lang="ts">
import { computed, ref } from "vue";
import { useDeliberation } from "@/stores/deliberation";
import { seatIdentity } from "@/lib/seat-identity";
import { useStickToBottom } from "@/lib/stick-to-bottom";
import StreamMarkdown from "@/components/StreamMarkdown.vue";
const store = useDeliberation();
const state = computed(() => store.running ? "SYSTEM ONLINE" : "SYSTEM IDLE");
const identity = computed(() => store.mediator ? seatIdentity(store.mediator) : "");
const scroller = ref<HTMLElement | null>(null);
const { onScroll } = useStickToBottom(scroller, () => store.mediatorText.length);
</script>

<template>
  <article class="terminal-panel min-h-[9rem] overflow-hidden border-cyan/45">
    <header class="flex items-center justify-between gap-2 border-b border-cyan/25 bg-cyan/7 px-4 py-2">
      <div class="min-w-0"><span class="font-mono text-sm text-cyan">MEDIATOR // {{ store.mediator?.display_name ?? 'UNASSIGNED' }}</span><span v-if="identity" class="ml-3 font-mono text-[9px] text-fg-faint">{{ identity }}</span></div>
      <span class="shrink-0 font-mono text-[9px] text-cyan">{{ state }}</span>
    </header>
    <div ref="scroller" class="max-h-40 overflow-y-auto px-4 py-3 font-mono text-[11px] leading-[1.7] text-foreground/90" @scroll.passive="onScroll"><StreamMarkdown v-if="store.mediatorText" :text="store.mediatorText" :streaming="store.running" cursor-class="text-cyan" /><p v-else class="text-fg-faint">&gt; mediator waits for panelist packets</p></div>
  </article>
</template>
