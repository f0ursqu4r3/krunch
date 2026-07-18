<script setup lang="ts">
import { computed } from "vue";
import { useDeliberation } from "@/stores/deliberation";
import StreamMarkdown from "@/components/StreamMarkdown.vue";
const store = useDeliberation();
const state = computed(() => store.running ? "SYSTEM ONLINE" : "SYSTEM IDLE");
</script>

<template>
  <article class="terminal-panel min-h-[9rem] overflow-hidden border-cyan/45">
    <header class="flex items-center justify-between border-b border-cyan/25 bg-cyan/7 px-4 py-2"><span class="font-mono text-sm text-cyan">MEDIATOR // {{ store.mediator?.display_name ?? 'UNASSIGNED' }}</span><span class="font-mono text-[9px] text-cyan">{{ state }}</span></header>
    <div class="max-h-40 overflow-y-auto px-4 py-3 font-mono text-[11px] leading-[1.7] text-foreground/90"><StreamMarkdown v-if="store.mediatorText" :text="store.mediatorText" :streaming="store.running" cursor-class="text-cyan" /><p v-else class="text-fg-faint">&gt; mediator waits for panelist packets</p></div>
  </article>
</template>
