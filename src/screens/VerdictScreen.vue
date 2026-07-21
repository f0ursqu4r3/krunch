<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { Copy, Download, RotateCcw, X } from "@lucide/vue";
import { useDeliberation } from "@/stores/deliberation";
import { isTauri } from "@/lib/api";
import { Button } from "@/components/ui/button";
import StreamMarkdown from "@/components/StreamMarkdown.vue";
const store = useDeliberation(); const ready = ref(false); const dismissed = ref(false); const copied = ref(false);
const saved = ref<string | null>(null); const exportError = ref<string | null>(null);
onMounted(() => { window.setTimeout(() => ready.value = true, matchMedia("(prefers-reduced-motion: reduce)").matches ? 0 : 650); });
const meta = computed(() => store.finalState === "converged" ? ["Consensus", "text-consensus"] : store.finalState === "deadlocked" ? ["Deadlock", "text-deadlock"] : [String(store.finalState ?? "Halted").replace(/^\w/, (c) => c.toUpperCase()), "text-signal"]);
const verdictText = computed(() => store.verdict?.text ?? store.failure?.reason ?? "No ruling was entered.");
async function copy() {
  exportError.value = null;
  try { await navigator.clipboard.writeText(await store.exportMarkdown()); copied.value = true; window.setTimeout(() => copied.value = false, 1200); }
  catch (error) { exportError.value = `copy failed: ${error}`; }
}
async function download() {
  exportError.value = null;
  try {
    if (isTauri()) {
      // Blob-anchor downloads are ignored by the WKWebView — save natively.
      saved.value = await store.saveDump();
    } else {
      const text = await store.exportMarkdown();
      const url = URL.createObjectURL(new Blob([text], { type: "text/markdown" }));
      const a = document.createElement("a"); a.href = url; a.download = "krunch-session-dump.md"; a.click();
      URL.revokeObjectURL(url); saved.value = "krunch-session-dump.md";
    }
  } catch (error) { exportError.value = `save failed: ${error}`; }
}
</script>

<template>
  <Transition name="fade" appear>
  <div v-if="!dismissed" class="absolute inset-0 z-30 grid place-items-center bg-bg-deep/85 p-5 backdrop-blur-sm" @click.self="ready = true">
    <section class="terminal-panel relative max-h-full w-full max-w-4xl overflow-y-auto border-2 p-6" :class="[ready ? 'boot' : '', store.finalState === 'deadlocked' ? 'border-deadlock' : 'border-signal']">
      <button class="absolute right-3 top-3 text-fg-faint hover:text-signal" aria-label="Dismiss verdict overlay" @click="dismissed = true"><X class="size-4" /></button>
      <p class="font-mono text-[10px] uppercase tracking-[0.2em] text-signal">DECODED TRANSMISSION</p>
      <pre v-if="!ready" class="mt-5 overflow-hidden font-mono text-xs leading-6 text-fg-muted">&gt; decoding panel transmission…
&gt; reconciling final confidence
&gt; writing to record <span class="cursor">_</span></pre>
      <template v-else><p class="mt-5 font-display text-4xl sm:text-6xl glow-text" :class="meta[1]">{{ meta[0] }}</p><p class="mt-3 font-mono text-[10px] text-fg-faint">{{ store.sessionId?.slice(0, 8) }} · {{ store.rounds.length }} rounds deliberated</p><article class="mt-7 border-y border-line py-6"><div class="max-w-[68ch] text-[15px] leading-[1.7] text-foreground/90"><StreamMarkdown :text="verdictText" /></div></article><footer class="mt-6 flex flex-wrap items-center gap-2"><Button size="sm" variant="outline" class="border-signal/45 text-signal" @click="copy"><Copy data-icon="inline-start" />{{ copied ? 'Copied' : 'Copy record' }}</Button><Button size="sm" variant="outline" class="border-consensus/45 text-consensus" @click="download"><Download data-icon="inline-start" />{{ saved ? 'Saved ✓' : 'Download filing' }}</Button><Button size="sm" variant="ghost" class="text-fg-muted" @click="store.backToSetup()"><RotateCcw data-icon="inline-start" />New session</Button></footer><p v-if="saved" class="mt-2 font-mono text-[10px] text-consensus">→ {{ saved }}</p><p v-if="exportError" class="mt-2 font-mono text-[10px] text-deadlock">! {{ exportError }}</p></template>
    </section>
  </div>
  </Transition>
</template>
