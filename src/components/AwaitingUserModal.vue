<script setup lang="ts">
import { reactive, watch } from "vue";
import { useDeliberation } from "@/stores/deliberation";

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
</script>

<template>
  <div v-if="store.awaiting" class="fixed inset-0 z-50 flex items-center justify-center bg-background/70 p-4 backdrop-blur-sm">
    <div class="w-full max-w-lg rounded-2xl border bg-card p-6 shadow-2xl">
      <h2 class="text-lg font-semibold">The mediator has questions</h2>
      <p class="mt-1 text-sm text-muted-foreground">
        Round {{ store.awaiting.round + 1 }} paused. Your answers are added to the shared context.
      </p>
      <div class="mt-4 space-y-4">
        <div v-for="(q, i) in store.awaiting.questions" :key="i">
          <label class="mb-1 block text-sm font-medium">{{ q }}</label>
          <textarea v-model="answers[i]" rows="2"
            class="w-full resize-y rounded-lg border bg-background px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-ring" />
        </div>
      </div>
      <div class="mt-6 flex justify-end gap-2">
        <button @click="store.abandon()" class="rounded-lg px-3 py-2 text-sm text-muted-foreground hover:text-foreground">
          Abandon
        </button>
        <button @click="submit"
          class="rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:opacity-90">
          Resume deliberation
        </button>
      </div>
    </div>
  </div>
</template>
