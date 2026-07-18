<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { useDeliberation } from "@/stores/deliberation";
import { useStickToBottom } from "@/lib/stick-to-bottom";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";

const store = useDeliberation();
const scroller = ref<HTMLElement | null>(null);
const { onScroll } = useStickToBottom(scroller, () => store.logLines[store.logLines.length - 1]?.id ?? 0);
const answers = reactive<Record<number, string>>({});
const localTime = (time: number) => new Date(time).toLocaleTimeString([], { hour12: false, hour: "2-digit", minute: "2-digit", second: "2-digit" });
const prompt = computed(() => store.awaiting);
watch(prompt, (value) => { if (value) value.questions.forEach((_, index) => answers[index] ??= ""); }, { immediate: true });
async function submit() {
  if (!prompt.value) return;
  await store.submitAnswers(prompt.value.questions.map((question, index) => [question, answers[index] ?? ""]));
  Object.keys(answers).forEach((key) => delete answers[Number(key)]);
}
</script>

<template>
  <aside class="log-rail terminal-panel flex min-h-0 flex-col overflow-hidden" aria-label="Event log">
    <header class="flex items-center justify-between border-b border-line px-3 py-2">
      <span class="font-mono text-xs text-cyan">EVENT LOG</span>
      <span class="font-mono text-[9px] text-fg-faint">{{ store.logLines.length }}/180</span>
    </header>
    <div ref="scroller" class="min-h-0 flex-1 overflow-y-auto p-3 font-mono text-[10px] leading-relaxed" @scroll.passive="onScroll">
      <p v-if="!store.logLines.length" class="text-fg-faint">[awaiting lifecycle events]</p>
      <p v-for="line in store.logLines" :key="line.id" class="break-words text-fg-muted"><span class="text-fg-faint">{{ localTime(line.receipt) }}</span> <span class="text-cyan">{{ line.kind }}</span> {{ line.text }}</p>
    </div>
    <form v-if="prompt" class="border-t border-brass/45 bg-brass/5 p-3" @submit.prevent="submit">
      <p class="mb-2 font-mono text-[11px] text-brass">OPERATOR INPUT // R{{ String(prompt.round + 1).padStart(2, '0') }}</p>
      <label v-for="(question, index) in prompt.questions" :key="index" class="mb-2 block text-[10px] text-foreground">{{ question }}<Textarea v-model="answers[index]" rows="2" class="mt-1 resize-none bg-bg-deep text-[11px]" /></label>
      <div class="flex gap-2"><Button size="xs" type="submit" class="bg-brass text-primary-foreground">Transmit answers</Button><Button size="xs" type="button" variant="ghost" class="text-deadlock" @click="store.abandon()">Abort</Button></div>
    </form>
  </aside>
</template>
