<script setup lang="ts">
import { computed, ref } from "vue";
import { useDeliberation } from "@/stores/deliberation";
import { seatIdentity } from "@/lib/seat-identity";
import { useStickToBottom } from "@/lib/stick-to-bottom";
import { useTypewriter } from "@/lib/typewriter";
import StreamMarkdown from "@/components/StreamMarkdown.vue";
const store = useDeliberation();
const state = computed(() => store.running ? "in session" : "adjourned");
const identity = computed(() => store.mediator ? seatIdentity(store.mediator) : "");
const { text: revealed, typing } = useTypewriter(() => store.mediatorText, () => store.running && !store.instantTokens);
const scroller = ref<HTMLElement | null>(null);
const { onScroll } = useStickToBottom(scroller, () => revealed.value.length);
</script>

<template>
  <article class="terminal-panel min-h-[9rem] overflow-hidden border-signal/45">
    <header class="flex items-center justify-between gap-2 border-b border-signal/25 bg-signal/7 px-4 py-2">
      <div class="flex min-w-0 items-baseline gap-2"><span class="font-mono text-[10px] uppercase tracking-[0.16em] text-signal">Mediator</span><span class="truncate font-display text-base text-foreground">{{ store.mediator?.display_name ?? 'unassigned' }}</span><span v-if="identity" class="hidden truncate font-mono text-[9px] text-fg-faint sm:inline">{{ identity }}</span></div>
      <span class="shrink-0 font-mono text-[9px] uppercase tracking-wide text-signal">{{ state }}</span>
    </header>
    <div ref="scroller" class="max-h-40 overflow-y-auto px-4 py-3 text-[13px] leading-[1.65] text-foreground/90" @scroll.passive="onScroll"><StreamMarkdown v-if="revealed" :text="revealed" :streaming="store.running" :typing="typing" cursor-class="text-signal" /><p v-else class="italic text-fg-faint">The mediator waits for the panel to speak.</p></div>
  </article>
</template>
