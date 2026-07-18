<script setup lang="ts">
import { computed, ref } from "vue";
import { useDeliberation } from "@/stores/deliberation";
import type { SessionState } from "@/lib/types";

const store = useDeliberation();
const copied = ref(false);

const outcome = computed<SessionState | null>(() => store.finalState);
const meta = computed(() => {
  switch (outcome.value) {
    case "converged":
      return { title: "Consensus reached", cls: "text-emerald-400", icon: "✓" };
    case "deadlocked":
      return { title: "Deadlocked", cls: "text-amber-400", icon: "⚖️" };
    case "halted":
      return { title: "Halted — too few panelists", cls: "text-destructive", icon: "⚠" };
    case "mediator_error":
      return { title: "Mediator error", cls: "text-destructive", icon: "⚠" };
    case "abandoned":
      return { title: "Abandoned", cls: "text-muted-foreground", icon: "✕" };
    case "interrupted":
      return { title: "Interrupted", cls: "text-muted-foreground", icon: "↻" };
    default:
      return { title: "Finished", cls: "text-foreground", icon: "•" };
  }
});

async function copyExport() {
  const md = await store.exportMarkdown();
  await navigator.clipboard.writeText(md);
  copied.value = true;
  setTimeout(() => (copied.value = false), 1800);
}
async function downloadExport() {
  const md = await store.exportMarkdown();
  const blob = new Blob([md], { type: "text/markdown" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = "krunch-deliberation.md";
  a.click();
  URL.revokeObjectURL(url);
}
</script>

<template>
  <div class="mx-auto max-w-3xl px-6 py-8">
    <header class="mb-6 flex items-center justify-between">
      <div class="flex items-center gap-3">
        <span class="text-2xl" :class="meta.cls">{{ meta.icon }}</span>
        <div>
          <h1 class="text-xl font-semibold" :class="meta.cls">{{ meta.title }}</h1>
          <p class="text-xs text-muted-foreground">{{ store.rounds.length }} rounds deliberated</p>
        </div>
      </div>
      <div class="flex gap-2">
        <button @click="copyExport" class="rounded-lg border px-3 py-2 text-xs hover:bg-accent">
          {{ copied ? "copied ✓" : "copy markdown" }}
        </button>
        <button @click="downloadExport" class="rounded-lg border px-3 py-2 text-xs hover:bg-accent">
          save .md
        </button>
        <button @click="store.backToSetup()" class="rounded-lg bg-primary px-3 py-2 text-xs font-medium text-primary-foreground hover:opacity-90">
          new deliberation
        </button>
      </div>
    </header>

    <!-- verdict -->
    <section v-if="store.verdict" class="rounded-2xl border bg-card p-6">
      <p class="whitespace-pre-wrap break-words text-sm leading-relaxed text-card-foreground/90">{{ store.verdict.text }}</p>
    </section>

    <!-- terminal failure -->
    <section v-else-if="store.failure" class="rounded-2xl border border-destructive/30 bg-destructive/5 p-6">
      <p class="text-sm text-muted-foreground">
        The deliberation ended in <span class="font-medium text-foreground">{{ store.failure.state }}</span> before a
        verdict was synthesized.
      </p>
      <p class="mt-2 text-sm text-destructive">{{ store.failure.reason }}</p>
      <p class="mt-4 text-xs text-muted-foreground">The transcript up to the last durable round is still exportable.</p>
    </section>

    <!-- convergence recap -->
    <section v-if="store.rounds.length" class="mt-4 text-xs text-muted-foreground">
      <span v-for="r in store.rounds" :key="r.round" class="mr-2 inline-flex items-center gap-1">
        <span class="inline-block h-2 w-2 rounded-full"
          :class="{
            'bg-emerald-500': r.ruling === 'CONSENSUS',
            'bg-primary': r.ruling === 'CONTINUE',
            'bg-destructive': r.ruling === 'DEADLOCK',
          }" />R{{ r.round + 1 }}
      </span>
    </section>
  </div>
</template>
