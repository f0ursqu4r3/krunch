<script setup lang="ts">
import { computed, ref, watch } from "vue";
import type { SeatConfig } from "@/lib/types";
import { useDeliberation } from "@/stores/deliberation";
import { measure } from "@/lib/pretext";
import { seatIdentity } from "@/lib/seat-identity";
import { useStickToBottom } from "@/lib/stick-to-bottom";
import { useTypewriter } from "@/lib/typewriter";
import StreamMarkdown from "@/components/StreamMarkdown.vue";

const props = defineProps<{ seat: SeatConfig; index: number }>();
const store = useDeliberation(); const lines = ref(0);
const live = computed(() => store.live[props.seat.id]);
watch(() => live.value?.text, async (text) => lines.value = text ? (await measure(text, 320)).lineCount : 0);
const state = computed(() => ({
  streaming: ["STREAM", "text-signal"], stance: ["LATCHED", "text-consensus"], abstained: ["ABSTAIN", "text-deadlock"], truncated: ["TRUNC", "text-signal"], idle: ["IDLE", "text-fg-faint"],
}[live.value?.status ?? "idle"]));
const ttft = computed(() => live.value?.firstTokenAt && live.value.startedAt ? `~${live.value.firstTokenAt - live.value.startedAt}ms TTFT` : "~— TTFT");
const identity = computed(() => seatIdentity(props.seat));

const { text: revealed, typing } = useTypewriter(
  () => live.value?.text ?? "",
  () => live.value?.status === "streaming" && !store.instantTokens,
);
const scroller = ref<HTMLElement | null>(null);
const { pinned, onScroll } = useStickToBottom(scroller, () => `${revealed.value.length}:${live.value?.reason ?? ""}`);
</script>

<template>
  <article :id="`seat-${index + 1}`" :data-seat-index="index" tabindex="-1" class="terminal-panel rise flex min-h-[12rem] min-w-0 flex-col overflow-hidden outline-none focus-visible:ring-2 focus-visible:ring-signal" :class="[live?.status === 'stance' ? 'border-consensus latch' : live?.status === 'abstained' ? 'border-deadlock' : live?.status === 'streaming' ? 'border-signal' : '', live?.streamIncomplete ? 'shake' : '']" :style="{ '--rise-delay': `${index * 45}ms` }">
    <header class="flex items-center justify-between gap-2 border-b border-line bg-bg-deep/40 px-3 py-2">
      <div class="min-w-0">
        <div class="flex min-w-0 items-baseline"><span class="mr-2 shrink-0 font-mono text-[11px] text-signal glow-text">[ CH{{ String(index + 1).padStart(2, '0') }} ]</span><span class="truncate font-sans text-[14px] tracking-wide text-foreground">{{ seat.display_name }}</span></div>
        <p v-if="identity" class="mt-0.5 truncate pl-7 font-mono text-[9px] text-fg-faint">{{ identity }}</p>
      </div>
      <span class="shrink-0 font-mono text-[9px]" :class="state[1]">{{ state[0] }}</span>
    </header>
    <div class="relative min-h-0 flex-1">
      <div ref="scroller" class="h-full overflow-y-auto px-3.5 py-3 text-[13px] leading-[1.6] text-foreground/90" @scroll.passive="onScroll">
        <StreamMarkdown v-if="revealed" :text="revealed" :streaming="live?.status === 'streaming'" :typing="typing" cursor-class="text-signal" />
        <p v-else class="italic text-fg-faint">Awaiting the seat's argument<span class="cursor">_</span></p>
        <p v-if="live?.reason" class="mt-2 text-deadlock">! {{ live.reason }}</p>
      </div>
      <Transition name="pop"><button v-if="!pinned && live?.status === 'streaming'" class="absolute bottom-2 right-3 border border-signal/45 bg-bg-deep/90 px-2 py-0.5 font-mono text-[9px] text-signal hover:bg-signal/15" @click="scroller && (scroller.scrollTop = scroller.scrollHeight)">▼ follow</button></Transition>
    </div>
    <footer class="border-t border-line px-3 py-2 font-mono text-[9px] text-fg-faint">
      <div class="flex justify-between gap-2"><span>{{ ttft }}</span><span>{{ live?.usage?.outputTokens === null || !live?.usage ? '~— tok/s' : `~${live.usage.outputTokens} out` }}</span></div>
      <div class="mt-1 flex justify-between gap-2"><span>{{ lines }} lines · {{ live?.usage ? `${live.usage.inputTokens ?? '?'} in / ${live.usage.outputTokens ?? '?'} out` : 'usage pending' }}</span><span v-if="live?.streamIncomplete" class="text-deadlock">⚠ stream incomplete</span></div>
      <p v-if="live?.stance" class="mt-2 border-t border-consensus/35 pt-2 text-consensus">STANCE: {{ live.stance }} // {{ Math.round((live.confidence ?? 0) * 100) }}%</p>
    </footer>
  </article>
</template>
