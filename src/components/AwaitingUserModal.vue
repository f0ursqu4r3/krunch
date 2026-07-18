<script setup lang="ts">
import { reactive, watch } from "vue";
import { useDeliberation } from "@/stores/deliberation";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";

const store = useDeliberation();
const answers = reactive<Record<number, string>>({});

watch(
  () => store.awaiting,
  (a) => {
    if (a) a.questions.forEach((_, i) => (answers[i] = answers[i] ?? ""));
  },
  { immediate: true },
);

async function submit() {
  const a = store.awaiting;
  if (!a) return;
  const pairs = a.questions.map((q, i) => [q, answers[i] ?? ""] as [string, string]);
  for (const k of Object.keys(answers)) delete answers[Number(k)];
  await store.submitAnswers(pairs);
}
const prevent = (e: Event) => e.preventDefault();
</script>

<template>
  <Dialog :open="!!store.awaiting">
    <DialogContent :show-close-button="false" @interact-outside="prevent" @escape-key-down="prevent"
      class="max-w-xl gap-0 border-brass/25 bg-surface p-0"
      style="box-shadow: 0 20px 80px -30px color-mix(in oklch, var(--brass) 55%, transparent);">
      <DialogHeader class="flex-row items-center gap-3 space-y-0 border-b border-brass/15 px-6 py-4">
        <span class="grid size-8 shrink-0 place-items-center rounded-full bg-brass/12 font-display text-brass ring-1 ring-brass/30">§</span>
        <div class="text-left">
          <DialogTitle class="font-display text-lg text-foreground">The foreman turns to you</DialogTitle>
          <DialogDescription class="font-mono text-[10px] uppercase tracking-[0.14em] text-brass/60">
            round {{ (store.awaiting?.round ?? 0) + 1 }} · deliberation paused
          </DialogDescription>
        </div>
      </DialogHeader>

      <div class="flex max-h-[50vh] flex-col gap-5 overflow-y-auto px-6 py-5">
        <div v-for="(q, i) in store.awaiting?.questions ?? []" :key="i" class="flex flex-col gap-2">
          <Label class="text-sm leading-snug text-foreground">{{ q }}</Label>
          <Textarea v-model="answers[i]" rows="2" placeholder="Your answer joins the record…"
            class="resize-y bg-bg-deep/50" />
        </div>
      </div>

      <DialogFooter class="items-center justify-between border-t border-line/60 px-6 py-4 sm:justify-between">
        <Button variant="ghost" size="sm" class="text-fg-faint hover:text-deadlock" @click="store.abandon()">
          Adjourn the panel
        </Button>
        <Button class="rounded-full bg-brass font-display text-primary-foreground hover:bg-brass-bright" @click="submit">
          Return to deliberation
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
