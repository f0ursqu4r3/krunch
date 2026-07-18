<script setup lang="ts">
import { computed, ref } from "vue";
import { useDeliberation } from "@/stores/deliberation";
import type { SessionState } from "@/lib/types";

const store = useDeliberation();
const copied = ref(false);

const outcome = computed<SessionState | null>(() => store.finalState);
const meta = computed(() => {
  switch (outcome.value) {
    case "converged": return { kicker: "The panel has ruled", title: "Consensus", tone: "text-consensus", glow: true };
    case "deadlocked": return { kicker: "The panel could not agree", title: "Deadlock", tone: "text-deadlock", glow: false };
    case "halted": return { kicker: "The chamber emptied", title: "Halted", tone: "text-deadlock", glow: false };
    case "mediator_error": return { kicker: "The foreman faltered", title: "Mistrial", tone: "text-deadlock", glow: false };
    case "abandoned": return { kicker: "You called it", title: "Adjourned", tone: "text-fg-muted", glow: false };
    case "interrupted": return { kicker: "The session broke off", title: "Interrupted", tone: "text-fg-muted", glow: false };
    default: return { kicker: "Concluded", title: "Verdict", tone: "text-foreground", glow: false };
  }
});

// Dependency-free, XSS-safe typesetting of the verdict Markdown: parse into
// blocks and render as real elements (text interpolation, never v-html).
type Block = { kind: "h"; text: string } | { kind: "p"; text: string } | { kind: "ul"; items: string[] };
const blocks = computed<Block[]>(() => {
  const src = store.verdict?.text ?? "";
  const out: Block[] = [];
  let para: string[] = [];
  let list: string[] = [];
  const flushPara = () => { if (para.length) { out.push({ kind: "p", text: para.join(" ") }); para = []; } };
  const flushList = () => { if (list.length) { out.push({ kind: "ul", items: list }); list = []; } };
  for (const raw of src.split("\n")) {
    const line = raw.trimEnd();
    if (/^#{1,6}\s/.test(line)) { flushPara(); flushList(); out.push({ kind: "h", text: line.replace(/^#{1,6}\s/, "") }); }
    else if (/^[-*]\s/.test(line)) { flushPara(); list.push(line.replace(/^[-*]\s/, "")); }
    else if (line.trim() === "") { flushPara(); flushList(); }
    else { flushList(); para.push(line); }
  }
  flushPara(); flushList();
  return out;
});

async function copyExport() {
  try {
    await navigator.clipboard.writeText(await store.exportMarkdown());
    copied.value = true;
    setTimeout(() => (copied.value = false), 1800);
  } catch { /* preview / no backend */ }
}
async function downloadExport() {
  try {
    const md = await store.exportMarkdown();
    const url = URL.createObjectURL(new Blob([md], { type: "text/markdown" }));
    const a = document.createElement("a");
    a.href = url; a.download = "krunch-ruling.md"; a.click();
    URL.revokeObjectURL(url);
  } catch { /* preview / no backend */ }
}
</script>

<template>
  <div class="h-full overflow-y-auto">
    <div class="mx-auto max-w-2xl px-8 pb-24 pt-16">
      <!-- Ruling head -->
      <header class="rise flex items-start justify-between gap-6">
        <div>
          <p class="font-mono text-[11px] uppercase tracking-[0.2em] text-fg-faint">{{ meta.kicker }}</p>
          <h1 class="mt-2 font-display text-6xl leading-none" :class="meta.tone"
            :style="meta.glow ? 'text-shadow: 0 0 50px color-mix(in oklch, var(--consensus) 45%, transparent)' : ''">
            {{ meta.title }}
          </h1>
          <p class="mt-3 font-mono text-xs text-fg-faint">
            {{ store.rounds.length }} {{ store.rounds.length === 1 ? "round" : "rounds" }} deliberated
          </p>
        </div>
        <button @click="store.backToSetup()"
          class="mt-1 shrink-0 rounded-full border border-line px-4 py-2 text-xs text-fg-muted transition hover:border-brass/50 hover:text-brass">
          Convene anew
        </button>
      </header>

      <!-- The ruling -->
      <section v-if="store.verdict" class="rise mt-10" style="animation-delay: 80ms">
        <div class="mb-5 flex items-center gap-3">
          <span class="h-px flex-1 bg-line" />
          <span class="font-mono text-[10px] uppercase tracking-[0.25em] text-brass/70">the ruling</span>
          <span class="h-px flex-1 bg-line" />
        </div>
        <article class="space-y-4">
          <template v-for="(b, i) in blocks" :key="i">
            <h3 v-if="b.kind === 'h'" class="pt-2 font-display text-xl text-brass">{{ b.text }}</h3>
            <p v-else-if="b.kind === 'p'" class="text-[15px] leading-[1.75] text-foreground/90">{{ b.text }}</p>
            <ul v-else class="space-y-2">
              <li v-for="(it, j) in b.items" :key="j" class="flex gap-3 text-[15px] leading-[1.7] text-foreground/85">
                <span class="mt-2.5 size-1 shrink-0 rounded-full bg-brass" />
                <span>{{ it }}</span>
              </li>
            </ul>
          </template>
        </article>
      </section>

      <!-- Terminal failure -->
      <section v-else-if="store.failure" class="rise mt-10 rounded-xl bg-surface/40 p-6 ring-1 ring-deadlock/20" style="animation-delay: 80ms">
        <p class="text-[15px] leading-relaxed text-fg-muted">
          The deliberation ended in <span class="text-foreground">{{ store.failure.state }}</span> before a verdict could be synthesized.
        </p>
        <p class="mt-3 font-mono text-sm text-deadlock">{{ store.failure.reason }}</p>
        <p class="mt-5 text-xs text-fg-faint">The record up to the last durable round remains on file.</p>
      </section>

      <!-- File the record -->
      <div class="rise mt-10 flex items-center gap-3" style="animation-delay: 140ms">
        <button @click="copyExport"
          class="rounded-full border border-line px-4 py-2 text-xs text-fg-muted transition hover:border-brass/50 hover:text-brass">
          {{ copied ? "copied to clipboard" : "copy the record" }}
        </button>
        <button @click="downloadExport"
          class="rounded-full border border-line px-4 py-2 text-xs text-fg-muted transition hover:border-brass/50 hover:text-brass">
          file as markdown
        </button>
      </div>
    </div>
  </div>
</template>
