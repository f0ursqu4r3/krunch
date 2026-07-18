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
  <Transition
    enter-active-class="transition duration-300 ease-[cubic-bezier(0.16,1,0.3,1)]"
    enter-from-class="opacity-0" enter-to-class="opacity-100">
    <div v-if="store.awaiting" class="fixed inset-0 z-50 flex items-end justify-center p-5"
      style="background: radial-gradient(80% 60% at 50% 100%, color-mix(in oklch, var(--brass) 10%, transparent), transparent), color-mix(in oklch, var(--bg-deep) 78%, transparent);">
      <Transition appear
        enter-active-class="transition duration-400 ease-[cubic-bezier(0.16,1,0.3,1)]"
        enter-from-class="translate-y-8 opacity-0" enter-to-class="translate-y-0 opacity-100">
        <div class="w-full max-w-xl overflow-hidden rounded-2xl border border-brass/25 bg-surface"
          style="box-shadow: 0 -20px 80px -30px color-mix(in oklch, var(--brass) 60%, transparent);">
          <div class="flex items-center gap-3 border-b border-brass/15 px-6 py-4">
            <span class="grid size-8 place-items-center rounded-full bg-brass/12 font-display text-brass ring-1 ring-brass/30">§</span>
            <div>
              <h2 class="font-display text-lg text-foreground">The foreman turns to you</h2>
              <p class="font-mono text-[10px] uppercase tracking-[0.14em] text-brass/60">
                round {{ store.awaiting.round + 1 }} · deliberation paused
              </p>
            </div>
          </div>

          <div class="max-h-[50vh] space-y-5 overflow-y-auto px-6 py-5">
            <div v-for="(q, i) in store.awaiting.questions" :key="i">
              <label class="mb-2 block text-sm leading-snug text-foreground">{{ q }}</label>
              <textarea v-model="answers[i]" rows="2" placeholder="Your answer joins the record…"
                class="w-full resize-y rounded-lg border border-line bg-bg-deep/50 px-3.5 py-2.5 text-sm text-foreground outline-none transition placeholder:text-fg-faint focus:border-brass/50 focus:ring-1 focus:ring-brass/30" />
            </div>
          </div>

          <div class="flex items-center justify-between gap-3 border-t border-line/60 px-6 py-4">
            <button @click="store.abandon()" class="text-xs text-fg-faint transition hover:text-deadlock">
              Adjourn the panel
            </button>
            <button @click="submit"
              class="rounded-full bg-brass px-5 py-2 font-display text-sm text-primary-foreground transition hover:bg-brass-bright">
              Return to deliberation
            </button>
          </div>
        </div>
      </Transition>
    </div>
  </Transition>
</template>
