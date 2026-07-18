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
const meta = computed(() => store.finalState === "converged" ? ["CONSENSUS LOCKED", "text-consensus"] : store.finalState === "deadlocked" ? ["DEADLOCK", "text-deadlock"] : [String(store.finalState ?? "HALTED").toUpperCase(), "text-brass"]);
const verdictText = computed(() => store.verdict?.text ?? store.failure?.reason ?? "No final packet received.");
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
  <div v-if="!dismissed" class="absolute inset-0 z-30 grid place-items-center bg-bg-deep/85 p-5 backdrop-blur-sm" @click.self="ready = true">
    <section class="terminal-panel relative max-h-full w-full max-w-4xl overflow-y-auto border-2 p-6" :class="ready ? 'boot' : ''">
      <button class="absolute right-3 top-3 text-fg-faint hover:text-cyan" aria-label="Dismiss verdict overlay" @click="dismissed = true"><X class="size-4" /></button>
      <p class="font-mono text-[10px] text-cyan">FINALIZATION // ASCII COMPILE</p>
      <pre v-if="!ready" class="mt-5 overflow-hidden text-brass">[##########----------] 48%\nloading accepted-completion packets\nchecking consensus guard\nrendering verdict artifact</pre>
      <template v-else><p class="mt-6 font-display text-4xl sm:text-6xl" :class="meta[1]">{{ meta[0] }}</p><p class="mt-2 font-mono text-[10px] text-fg-faint">SESSION {{ store.sessionId?.slice(0, 8) }} · {{ store.rounds.length }} deliberated rounds · accepted-completion tokens</p><article class="mt-6 border-y border-line py-5 font-mono text-xs leading-7 text-foreground/90"><StreamMarkdown :text="verdictText" /></article><footer class="mt-5 flex flex-wrap items-center gap-2"><Button size="sm" variant="outline" class="border-cyan/45 text-cyan" @click="copy"><Copy data-icon="inline-start" />{{ copied ? 'Copied' : 'Copy dump' }}</Button><Button size="sm" variant="outline" class="border-consensus/45 text-consensus" @click="download"><Download data-icon="inline-start" />{{ saved ? 'Saved ✓' : 'Download dump' }}</Button><Button size="sm" variant="ghost" class="text-fg-muted" @click="store.backToSetup()"><RotateCcw data-icon="inline-start" />New session</Button></footer><p v-if="saved" class="mt-2 font-mono text-[10px] text-consensus">→ {{ saved }}</p><p v-if="exportError" class="mt-2 font-mono text-[10px] text-deadlock">! {{ exportError }}</p></template>
    </section>
  </div>
</template>
